//! GNU gettext PO filter. Parse/write follow Java `PoFilter`.

use crate::{
    ensure_parent, extract_tags, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile,
    ProtectedPart, Result,
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
    fuzzy_prev: Option<String>,
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
        if let Some(prev) = &e.fuzzy_prev {
            if !prev.is_empty() {
                segments.push(ExtractedSegment {
                    id: format!("{i}-fuzzy-prev"),
                    source: prev.clone(),
                    existing_translation: e.msgstr.first().cloned().filter(|s| !s.is_empty()),
                    note: None,
                    comment: Some("reference".into()),
                    path: e.msgctxt.clone(),
                    protected_parts: vec![],
                });
            }
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
    let _ = skip_header;
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
    let mut fuzzy_field = String::new();

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("#| msgid ") {
            fuzzy_field = "msgid".into();
            cur.fuzzy_prev = Some(unquote(rest));
            continue;
        }
        if line.starts_with("#| msgid") && line.contains('"') {
            fuzzy_field = "msgid".into();
            if let Some(q) = line.find('"') {
                cur.fuzzy_prev = Some(unquote(&line[q..]));
            }
            continue;
        }
        if line.starts_with("#|") && line.contains('"') && fuzzy_field == "msgid" {
            if let Some(s) = &mut cur.fuzzy_prev {
                if let Some(q) = line.find('"') {
                    s.push_str(&unquote(&line[q..]));
                }
            }
            continue;
        }
        if line.starts_with('#') {
            cur.comments.push(line.to_string());
            continue;
        }
        if line.trim().is_empty() {
            if started {
                entries.push(std::mem::take(&mut cur));
                field.clear();
                fuzzy_field.clear();
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

/// Java `PoFilter` write: drop `#, fuzzy` and `#|` lines; bilingual blank msgstr is empty.
fn rewrite_po(raw: &str, translations: &HashMap<String, String>, ctx: &FilterContext) -> String {
    let entries = collect_entries(raw);
    let nplurals = nplurals_from_header(&entries);
    let monolingual = ctx.option_flag("monolingualFormat");
    let mut out = String::new();
    for e in &entries {
        for c in &e.comments {
            let t = c.trim();
            if t.starts_with("#,") && t.contains("fuzzy") {
                continue;
            }
            if t.starts_with("#|") {
                continue;
            }
            out.push_str(c);
            out.push('\n');
        }
        if let Some(ctxv) = &e.msgctxt {
            out.push_str("msgctxt ");
            out.push_str(&quote(ctxv));
            out.push('\n');
        }
        out.push_str("msgid ");
        out.push_str(&quote(&e.msgid));
        out.push('\n');
        if let Some(pl) = &e.msgid_plural {
            out.push_str("msgid_plural ");
            out.push_str(&quote(pl));
            out.push('\n');
            for i in 0..nplurals {
                let src = if i == 0 {
                    e.msgid.as_str()
                } else {
                    pl.as_str()
                };
                let t = lookup_tr(translations, src, i);
                out.push_str(&format!("msgstr[{i}] "));
                out.push_str(&quote(&t));
                out.push('\n');
            }
        } else {
            let t = if e.msgid.is_empty() {
                e.msgstr.first().cloned().unwrap_or_default()
            } else if monolingual {
                lookup_tr(translations, e.msgstr.first().map(|s| s.as_str()).unwrap_or(""), 0)
            } else {
                lookup_tr(translations, &e.msgid, 0)
            };
            out.push_str("msgstr ");
            out.push_str(&quote(&t));
            out.push('\n');
        }
        out.push('\n');
    }
    if out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn lookup_tr(translations: &HashMap<String, String>, source: &str, _plural: usize) -> String {
    translations
        .get(source)
        .cloned()
        .or_else(|| translations.get(&source.to_string()).cloned())
        .unwrap_or_default()
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
