use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TmxEntry {
    pub source: String,
    pub translation: String,
    pub creator: Option<String>,
    pub created: Option<String>,
    pub changer: Option<String>,
    pub changed: Option<String>,
    pub note: Option<String>,
    pub default_translation: bool,
    pub file: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTmx {
    pub entries: Vec<TmxEntry>,
    by_source: HashMap<String, usize>,
}

impl ProjectTmx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path, source_lang: &str, target_lang: &str) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(path)?;
        Ok(parse_tmx(&raw, source_lang, target_lang))
    }

    pub fn insert(&mut self, entry: TmxEntry) {
        if let Some(&idx) = self.by_source.get(&entry.source) {
            self.entries[idx] = entry;
        } else {
            self.by_source
                .insert(entry.source.clone(), self.entries.len());
            self.entries.push(entry);
        }
    }

    pub fn get(&self, source: &str) -> Option<&TmxEntry> {
        self.by_source.get(source).map(|&i| &self.entries[i])
    }

    pub fn write(&self, path: &Path, source_lang: &str, target_lang: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let bak = path.with_extension("tmx.bak");
            let _ = std::fs::copy(path, bak);
        }
        std::fs::write(path, self.to_xml(source_lang, target_lang))?;
        Ok(())
    }

    pub fn to_xml(&self, source_lang: &str, target_lang: &str) -> String {
        self.to_xml_level(source_lang, target_lang, "omegat")
    }

    pub fn to_xml_level(&self, source_lang: &str, target_lang: &str, level: &str) -> String {
        let mut body = String::new();
        for e in &self.entries {
            if e.translation.is_empty() {
                continue;
            }
            let src = if level == "level1" {
                strip_tags(&e.source)
            } else {
                e.source.clone()
            };
            let tgt = if level == "level1" {
                strip_tags(&e.translation)
            } else {
                e.translation.clone()
            };
            body.push_str("    <tu>\n");
            if level == "omegat" {
                if let Some(note) = &e.note {
                    if !note.is_empty() {
                        body.push_str(&format!(
                            "      <note>{}</note>\n",
                            xml_escape(note)
                        ));
                    }
                }
            }
            body.push_str(&format!(
                "      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
                xml_escape(source_lang),
                xml_escape(&src)
            ));
            body.push_str(&format!(
                "      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
                xml_escape(target_lang),
                xml_escape(&tgt)
            ));
            body.push_str("    </tu>\n");
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE tmx SYSTEM "tmx14.dtd">
<tmx version="1.4">
  <header creationtool="OmegaT" creationtoolversion="{ver}" segtype="sentence" o-tmf="OmegaT TMX" adminlang="EN-US" srclang="{src}" datatype="plaintext"/>
  <body>
{body}  </body>
</tmx>
"#,
            ver = omegat_ipc::APP_VERSION,
            src = xml_escape(source_lang),
            body = body
        )
    }
}

pub fn parse_tmx(raw: &str, source_lang: &str, target_lang: &str) -> ProjectTmx {
    let src_l = source_lang.to_ascii_lowercase();
    let tgt_l = target_lang.to_ascii_lowercase();
    let mut tmx = ProjectTmx::new();
    let mut rest = raw;
    while let Some(tu_start) = rest.find("<tu") {
        let slice = &rest[tu_start..];
        let tu_end = slice.find("</tu>").unwrap_or(slice.len());
        let tu = &slice[..tu_end];
        let note = extract_tag(tu, "note");
        let mut source = None;
        let mut translation = None;
        let mut search = tu;
        while let Some(p) = search.find("<tuv") {
            let tuv = &search[p..];
            let end = tuv.find("</tuv>").unwrap_or(tuv.len());
            let block = &tuv[..end];
            let lang = attr(block, "xml:lang")
                .or_else(|| attr(block, "lang"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            let seg = extract_tag(block, "seg").unwrap_or_default();
            if lang_matches(&lang, &src_l) && source.is_none() {
                source = Some(seg);
            } else if lang_matches(&lang, &tgt_l) {
                translation = Some(seg);
            } else if source.is_some() && translation.is_none() && !lang_matches(&lang, &src_l) {
                translation = Some(seg);
            }
            search = &search[p + 4..];
        }
        if let (Some(s), Some(t)) = (source, translation) {
            tmx.insert(TmxEntry {
                source: s,
                translation: t,
                note,
                default_translation: true,
                ..Default::default()
            });
        }
        rest = &rest[tu_start + 3..];
    }
    tmx
}

fn lang_matches(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.starts_with(b) || b.starts_with(a) || a.split(['-', '_']).next() == b.split(['-', '_']).next()
}

fn extract_tag(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = raw.find(&open)?;
    let after = &raw[start..];
    let gt = after.find('>')? + start + 1;
    let close = format!("</{tag}>");
    let end = raw[gt..].find(&close)? + gt;
    Some(html_escape::decode_html_entities(&raw[gt..end]).into_owned())
}

fn attr(block: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let s = block.find(&key)? + key.len();
    let e = block[s..].find('"')? + s;
    Some(block[s..e].to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn strip_tags(s: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_tmx() {
        let raw = r#"<tmx><body>
        <tu><tuv lang="en"><seg>Hello</seg></tuv><tuv lang="fr"><seg>Bonjour</seg></tuv></tu>
        </body></tmx>"#;
        let tmx = parse_tmx(raw, "en", "fr");
        assert_eq!(tmx.get("Hello").unwrap().translation, "Bonjour");
    }
}
