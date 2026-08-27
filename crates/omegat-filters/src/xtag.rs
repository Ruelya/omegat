//! Java `org.omegat.filters2.xtagqxp.XtagFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct XtagFilter;

impl Filter for XtagFilter {
    fn id(&self) -> &'static str {
        "xtag"
    }
    fn name(&self) -> &'static str {
        "QuarkXPress CopyFlow Gold"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xtg", "*.tag"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

struct Xtag {
    tag: String,
    index: i32,
}

impl Xtag {
    fn shortcut_letter(&self) -> String {
        for c in self.tag.chars() {
            if c.is_alphabetic() {
                return c.to_lowercase().to_string();
            }
        }
        if self.tag.ends_with('<') {
            "<".into()
        } else if self.tag.ends_with('>') {
            ">".into()
        } else {
            "x".into()
        }
    }

    fn to_shortcut(&self) -> String {
        let s = self.shortcut_letter();
        if s == "<" || s == ">" {
            return s;
        }
        format!("<{}{}/>", s, self.index)
    }

    fn to_original(&self) -> String {
        format!("<{}>", self.tag)
    }
}

const EOL: &str = "\r\n";

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let mut segments = Vec::new();
    let mut written = String::new();
    let lines: Vec<&str> = crate::text::lines_with_breaks(raw)
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    let mut state_read = false;
    for (i, s) in lines.iter().enumerate() {
        let mut line = (*s).to_string();
        if line.starts_with("@$:") {
            written.push_str("@$:");
            line = line[3..].to_string();
            state_read = true;
        } else if line.starts_with("#boxname") {
            state_read = false;
        }
        if state_read {
            let (source, tags) = convert_to_tags(&line);
            if !source.is_empty() {
                segments.push(seg(segments.len().to_string(), &source));
            }
            let trans = if let Some(map) = translations {
                map.get(&source).cloned().unwrap_or_else(|| source.clone())
            } else {
                source
            };
            written.push_str(&convert_to_xtags(&trans, &tags));
        } else {
            written.push_str(&line);
        }
        if i + 1 < lines.len() {
            written.push_str(EOL);
        }
    }
    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn convert_to_tags(s: &str) -> (String, Vec<Xtag>) {
    let mut out = String::new();
    let mut tag = String::new();
    let mut collecting = false;
    let mut num = 0i32;
    let mut tags = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let cp = chars[i];
        if cp == '<' && !collecting {
            tag.clear();
            collecting = true;
        } else if cp == '>' && collecting {
            let last_is_slash = tag.chars().next_back() == Some('\\');
            if last_is_slash {
                tag.push(cp);
            } else {
                num += 1;
                let one = Xtag {
                    tag: tag.clone(),
                    index: num,
                };
                out.push_str(&one.to_shortcut());
                tags.push(one);
                tag.clear();
                collecting = false;
            }
        } else if collecting {
            tag.push(cp);
        } else {
            out.push(cp);
        }
        i += 1;
    }
    (out, tags)
}

fn convert_to_xtags(s: &str, tags: &[Xtag]) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut collecting = false;
    for cp in s.chars() {
        if cp == '<' && !collecting {
            tag.clear();
            tag.push(cp);
            collecting = true;
        } else if cp == '>' && collecting {
            tag.push(cp);
            out.push_str(&find_tag(&tag, tags));
            collecting = false;
            tag.clear();
        } else if collecting {
            tag.push(cp);
        } else {
            out.push_str(&convert_special(cp));
        }
    }
    if !tag.is_empty() {
        out.push_str(&find_tag(&tag, tags));
    }
    out
}

fn find_tag(tag: &str, tags: &[Xtag]) -> String {
    let inner = tag.trim_start_matches('<').trim_end_matches('>');
    for one in tags {
        if inner
            == one
                .to_shortcut()
                .trim_start_matches('<')
                .trim_end_matches('>')
            || tag == one.to_shortcut()
        {
            return one.to_original();
        }
    }
    let mut changed = String::new();
    for cp in tag.chars() {
        changed.push_str(&convert_special(cp));
    }
    changed
}

fn convert_special(cp: char) -> String {
    match cp {
        '<' => "<\\<>".into(),
        '>' => "<\\>>".into(),
        _ => cp.to_string(),
    }
}
