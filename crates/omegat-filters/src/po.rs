use crate::{
    ensure_parent, extract_tags, read_to_string, ExtractedSegment, Filter, FilterContext,
    ParsedFile, ProtectedPart, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct PoFilter;

impl Filter for PoFilter {
    fn id(&self) -> &'static str {
        "po"
    }
    fn name(&self) -> &'static str {
        "PO"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.po", "*.pot"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        parse_po(&read_to_string(path)?, ctx)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let out = rewrite_po(&raw, translations, ctx);
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

#[derive(Default, Clone)]
struct PoEntry {
    comments: Vec<String>,
    msgctxt: Option<String>,
    msgid: String,
    msgid_plural: Option<String>,
    msgstr: Vec<String>,
}

fn nplurals_from_header(entries: &[PoEntry]) -> usize {
    for e in entries {
        if e.msgid.is_empty() {
            let hdr = e.msgstr.first().cloned().unwrap_or_default();
            if let Some(rest) = hdr.split("nplurals=").nth(1) {
                let n: usize = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(2);
                return n.max(1);
            }
        }
    }
    2
}

fn parse_po(raw: &str, ctx: &FilterContext) -> Result<ParsedFile> {
    let entries = collect_entries(raw);
    let skip_header = ctx.option_flag("skipHeader");
    let monolingual = ctx.option_flag("monolingualFormat");
    let nplurals = nplurals_from_header(&entries);
    let mut segments = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if e.msgid.is_empty() {
            if skip_header || monolingual {
                continue;
            }
            continue;
        }
        if monolingual {
            let source = e.msgstr.first().cloned().unwrap_or_default();
            segments.push(ExtractedSegment {
                id: e.msgid.clone(),
                source,
                existing_translation: None,
                note: None,
                comment: None,
                path: e.msgctxt.clone(),
                protected_parts: vec![],
            });
            continue;
        }
        let existing = e.msgstr.first().cloned().filter(|s| !s.is_empty());
        let tags = extract_tags(&e.msgid);
        segments.push(ExtractedSegment {
            id: i.to_string(),
            source: e.msgid.clone(),
            existing_translation: existing,
            note: None,
            comment: if e.comments.is_empty() {
                None
            } else {
                Some(e.comments.join("\n"))
            },
            path: e.msgctxt.clone(),
            protected_parts: tags
                .into_iter()
                .map(|t| ProtectedPart {
                    text: t,
                    details: "tag".into(),
                })
                .collect(),
        });
        if e.msgid_plural.is_some() {
            for p in 1..nplurals {
                let src = e.msgid_plural.clone().unwrap_or_else(|| e.msgid.clone());
                segments.push(ExtractedSegment {
                    id: format!("{i}-plural-{p}"),
                    source: src,
                    existing_translation: e.msgstr.get(p).cloned().filter(|s| !s.is_empty()),
                    note: None,
                    comment: None,
                    path: e.msgctxt.as_ref().map(|c| format!("{c}[{p}]")),
                    protected_parts: vec![],
                });
            }
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

fn collect_entries(raw: &str) -> Vec<PoEntry> {
    let mut entries = Vec::new();
    let mut cur = PoEntry::default();
    let mut field = String::new();
    let mut started = false;

    for line in raw.lines() {
        if line.starts_with('#') {
            if started && !cur.msgid.is_empty() && !field.is_empty() && line.starts_with("#,") {
                // continue
            }
            cur.comments.push(line.to_string());
            continue;
        }
        if line.trim().is_empty() {
            if started {
                entries.push(std::mem::take(&mut cur));
                field.clear();
                started = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgctxt ") {
            started = true;
            field = "msgctxt".into();
            cur.msgctxt = Some(unquote(rest));
        } else if let Some(rest) = line.strip_prefix("msgid_plural ") {
            started = true;
            field = "msgid_plural".into();
            cur.msgid_plural = Some(unquote(rest));
        } else if let Some(rest) = line.strip_prefix("msgid ") {
            started = true;
            field = "msgid".into();
            cur.msgid = unquote(rest);
        } else if let Some(rest) = line.strip_prefix("msgstr") {
            started = true;
            field = "msgstr".into();
            let v = rest.find(' ').map(|i| unquote(&rest[i + 1..])).unwrap_or_default();
            cur.msgstr.push(v);
        } else if line.starts_with('"') {
            let extra = unquote(line);
            match field.as_str() {
                "msgctxt" => {
                    if let Some(s) = &mut cur.msgctxt {
                        s.push_str(&extra);
                    }
                }
                "msgid" => cur.msgid.push_str(&extra),
                "msgid_plural" => {
                    if let Some(s) = &mut cur.msgid_plural {
                        s.push_str(&extra);
                    }
                }
                "msgstr" => {
                    if let Some(last) = cur.msgstr.last_mut() {
                        last.push_str(&extra);
                    }
                }
                _ => {}
            }
        }
    }
    if started || !cur.msgid.is_empty() || !cur.comments.is_empty() {
        entries.push(cur);
    }
    entries
}

fn rewrite_po(raw: &str, translations: &HashMap<String, String>, ctx: &FilterContext) -> String {
    let parsed = parse_po(raw, ctx).ok();
    let mut by_source: HashMap<String, String> = HashMap::new();
    if let Some(p) = &parsed {
        for (i, seg) in p.segments.iter().enumerate() {
            if let Some(t) = translations
                .get(&seg.id)
                .or_else(|| translations.get(&seg.source))
                .or_else(|| translations.get(&i.to_string()))
            {
                if !t.is_empty() {
                    by_source.insert(seg.source.clone(), t.clone());
                    by_source.insert(seg.id.clone(), t.clone());
                }
            }
        }
    }
    let entries = collect_entries(raw);
    let mut out = String::new();
    let mut idx = 0usize;
    let mut in_msgstr = false;
    let mut skipping_msgstr_cont = false;
    for line in raw.lines() {
        if line.starts_with("msgid ") {
            in_msgstr = false;
            skipping_msgstr_cont = false;
            out.push_str(line);
            out.push('\n');
        } else if line.starts_with("msgstr[") {
            in_msgstr = true;
            skipping_msgstr_cont = true;
            out.push_str(line);
            out.push('\n');
        } else if line.starts_with("msgstr ") {
            in_msgstr = true;
            skipping_msgstr_cont = true;
            let msgid = entries.get(idx).map(|e| e.msgid.as_str()).unwrap_or("");
            let original = entries
                .get(idx)
                .and_then(|e| e.msgstr.first())
                .cloned()
                .unwrap_or_default();
            let t = translations
                .get(&idx.to_string())
                .cloned()
                .or_else(|| by_source.get(msgid).cloned())
                .or_else(|| translations.get(msgid).cloned())
                .unwrap_or_else(|| original.clone());
            if msgid.is_empty() {
                out.push_str(line);
                out.push('\n');
                skipping_msgstr_cont = false;
            } else {
                out.push_str("msgstr ");
                out.push_str(&quote(&t));
                out.push('\n');
            }
            idx += 1;
        } else if in_msgstr && line.starts_with('"') && skipping_msgstr_cont {
            continue;
        } else {
            if line.trim().is_empty() {
                in_msgstr = false;
                skipping_msgstr_cont = false;
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
