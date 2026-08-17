//! XML dialect tables ported from Java `org.omegat.filters3.xml.*Dialect`.
//! Paragraph tags become segments (including inline children as shortcuts).
//! Intact tags and `translatable="false"` are skipped. Write-back replaces
//! the matching paragraph node's inner content, not the first file-wide find.

use crate::{
    ensure_parent, extract_tags, read_to_string, ExtractedSegment, FilterContext, ParsedFile,
    ProtectedPart, Result,
};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy)]
pub struct XmlDialect {
    pub paragraph: &'static [&'static str],
    pub intact: &'static [&'static str],
    pub id_attrs: &'static [&'static str],
    pub skip_attr_false: &'static [&'static str],
    pub inline_shortcuts: bool,
}

impl XmlDialect {
    pub const fn new(paragraph: &'static [&'static str]) -> Self {
        Self {
            paragraph,
            intact: &[],
            id_attrs: &["name", "id", "key"],
            skip_attr_false: &[],
            inline_shortcuts: true,
        }
    }
}

pub fn parse_dialect(raw: &str, d: XmlDialect) -> Result<ParsedFile> {
    let doc = match roxmltree::Document::parse(raw) {
        Ok(d) => d,
        Err(_) => return fallback_scan(raw, d),
    };
    let mut segments = Vec::new();
    walk(doc.root_element(), d, &mut segments, &[]);
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

fn walk(
    node: roxmltree::Node<'_, '_>,
    d: XmlDialect,
    segments: &mut Vec<ExtractedSegment>,
    path: &[&str],
) {
    if !node.is_element() {
        return;
    }
    let name = node.tag_name().name();
    if is_named(name, d.intact) {
        return;
    }
    if is_named(name, d.paragraph) && !skip_untranslatable(node, d) {
        let source = collect_source(node, d);
        let source = normalize_android_escapes(&source);
        if !source.trim().is_empty() {
            let id = d
                .id_attrs
                .iter()
                .find_map(|a| node.attribute(*a))
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let parent = path.last().copied().unwrap_or("");
                    if !parent.is_empty() {
                        format!("{parent}/{name}/{}", segments.len())
                    } else {
                        segments.len().to_string()
                    }
                });
            let tags = extract_tags(&source);
            segments.push(ExtractedSegment {
                id,
                source,
                existing_translation: None,
                note: None,
                comment: None,
                path: Some(name.to_string()),
                protected_parts: tags
                    .into_iter()
                    .map(|t| ProtectedPart {
                        text: t,
                        details: "tag".into(),
                    })
                    .collect(),
            });
        }
        // Nested paragraph tags (e.g. Android item inside string) still walk children
        // only when this node itself was not a leaf paragraph with only inline kids.
        for child in node.children() {
            if child.is_element() && is_named(child.tag_name().name(), d.paragraph) {
                let mut next = path.to_vec();
                next.push(name);
                walk(child, d, segments, &next);
            }
        }
        return;
    }
    let mut next = path.to_vec();
    next.push(name);
    for child in node.children() {
        walk(child, d, segments, &next);
    }
}

fn skip_untranslatable(node: roxmltree::Node<'_, '_>, d: XmlDialect) -> bool {
    for attr in d.skip_attr_false {
        if node
            .attribute(*attr)
            .map(|v| v.eq_ignore_ascii_case("false"))
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn collect_source(node: roxmltree::Node<'_, '_>, d: XmlDialect) -> String {
    let mut out = String::new();
    let mut inline_i = 0usize;
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            let cname = child.tag_name().name();
            if is_named(cname, d.paragraph) {
                continue;
            }
            if is_named(cname, d.intact) {
                continue;
            }
            if d.inline_shortcuts {
                let inner = collect_source(child, d);
                let letter = if cname.eq_ignore_ascii_case("g") {
                    'x'
                } else {
                    cname
                        .chars()
                        .find(|c| c.is_ascii_alphabetic())
                        .unwrap_or('f')
                        .to_ascii_lowercase()
                };
                out.push_str(&format!("<{letter}{inline_i}>{inner}</{letter}{inline_i}>"));
                inline_i += 1;
            } else {
                out.push_str(&collect_source(child, d));
            }
        }
    }
    collapse_ws(&out)
}

fn collapse_ws(s: &str) -> String {
    let t = s.replace('\r', "");
    let re = Regex::new(r"[ \t]*\n[ \t]*").unwrap();
    let t = re.replace_all(&t, "\n");
    t.trim().to_string()
}

fn normalize_android_escapes(s: &str) -> String {
    s.replace("\\'", "'").replace("\\\"", "\"")
}

fn is_named(name: &str, tags: &[&str]) -> bool {
    tags.iter()
        .any(|t| *t == name || name.eq_ignore_ascii_case(t) || t.ends_with(&format!(":{name}")))
}

fn fallback_scan(raw: &str, d: XmlDialect) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    for tag in d.paragraph {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        let mut pos = 0usize;
        while let Some(rel) = raw[pos..].find(&open) {
            let start = pos + rel;
            let after = &raw[start..];
            let Some(gt) = after.find('>') else { break };
            let content_start = start + gt + 1;
            let Some(rel_end) = raw[content_start..].find(&close) else {
                break;
            };
            let content = raw[content_start..content_start + rel_end].trim();
            if !content.is_empty() {
                segments.push(ExtractedSegment {
                    id: segments.len().to_string(),
                    source: html_escape::decode_html_entities(content).into_owned(),
                    existing_translation: None,
                    note: None,
                    comment: None,
                    path: Some((*tag).to_string()),
                    protected_parts: vec![],
                });
            }
            pos = content_start + rel_end + close.len();
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

/// Replace each paragraph node's inner content by source-tree range (not file-wide find).
pub fn write_dialect(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    d: XmlDialect,
    ctx: &FilterContext,
) -> Result<()> {
    let _ = ctx;
    let raw = read_to_string(source_path)?;
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    if let Ok(doc) = roxmltree::Document::parse(&raw) {
        collect_replacements(doc.root_element(), d, translations, &mut replacements, &[]);
    }
    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = raw;
    for (start, end, text) in replacements {
        if start < end && end <= out.len() {
            out.replace_range(start..end, &text);
        }
    }
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, out)?;
    Ok(())
}

fn collect_replacements(
    node: roxmltree::Node<'_, '_>,
    d: XmlDialect,
    translations: &HashMap<String, String>,
    out: &mut Vec<(usize, usize, String)>,
    path: &[&str],
) {
    if !node.is_element() {
        return;
    }
    let name = node.tag_name().name();
    if is_named(name, d.intact) {
        return;
    }
    if is_named(name, d.paragraph) && !skip_untranslatable(node, d) {
        let source = normalize_android_escapes(&collect_source(node, d));
        if !source.trim().is_empty() {
            let id = d
                .id_attrs
                .iter()
                .find_map(|a| node.attribute(*a))
                .map(|s| s.to_string())
                .unwrap_or_else(|| path.last().copied().unwrap_or("").to_string());
            if let Some(t) = translations
                .get(&id)
                .or_else(|| translations.get(&source))
                .filter(|t| !t.is_empty() && *t != &source)
            {
                if let Some((start, end)) = inner_range(node) {
                    out.push((start, end, html_escape::encode_text(t).to_string()));
                }
            }
        }
        for child in node.children() {
            if child.is_element() && is_named(child.tag_name().name(), d.paragraph) {
                let mut next = path.to_vec();
                next.push(name);
                collect_replacements(child, d, translations, out, &next);
            }
        }
        return;
    }
    let mut next = path.to_vec();
    next.push(name);
    for child in node.children() {
        collect_replacements(child, d, translations, out, &next);
    }
}

fn inner_range(node: roxmltree::Node<'_, '_>) -> Option<(usize, usize)> {
    let range = node.range();
    let raw = node.document().input_text();
    let slice = raw.get(range.start..range.end)?;
    let inner_start = range.start + slice.find('>')? + 1;
    let inner_end = range.start + slice.rfind("</")?;
    if inner_start <= inner_end {
        Some((inner_start, inner_end))
    } else {
        None
    }
}

pub const ANDROID: XmlDialect = XmlDialect {
    paragraph: &["string", "item"],
    intact: &[],
    id_attrs: &["name", "id"],
    skip_attr_false: &["translatable", "translate"],
    inline_shortcuts: true,
};

pub const XHTML: XmlDialect = XmlDialect {
    paragraph: &[
        "title", "p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th", "dt", "dd", "address",
        "blockquote", "caption", "div",
    ],
    intact: &["style", "script", "object", "embed"],
    id_attrs: &["id", "name"],
    skip_attr_false: &[],
    inline_shortcuts: true,
};

pub const PROPXML: XmlDialect = XmlDialect::new(&["entry"]);
pub const RESX: XmlDialect = XmlDialect {
    paragraph: &["value"],
    intact: &["resheader", "metadata", "comment"],
    id_attrs: &["name", "id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
pub const WIX: XmlDialect = XmlDialect::new(&["String"]);
pub const SVG: XmlDialect = XmlDialect {
    paragraph: &["text", "p", "flowRoot", "tspan", "title", "desc"],
    intact: &["style", "image", "path"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: true,
};
pub const HELPANDMANUAL: XmlDialect = XmlDialect::new(&["caption", "text", "para"]);
pub const SCHEMATRON: XmlDialect = XmlDialect {
    paragraph: &["assert", "report"],
    intact: &["phase", "active", "ns", "include", "key", "let"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
pub const RELAXNG: XmlDialect = XmlDialect {
    paragraph: &["documentation"],
    intact: &["value", "name", "nsName"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
pub const CAMTASIA: XmlDialect = XmlDialect::new(&["caption", "title"]);
pub const TYPO3: XmlDialect = XmlDialect::new(&["title", "subtitle", "p", "header", "li", "td", "abstract"]);
pub const L10NMGR: XmlDialect = XmlDialect {
    paragraph: &["data"],
    intact: &["head"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
pub const INFIX: XmlDialect = XmlDialect::new(&["STORY", "P"]);
pub const FLASH: XmlDialect = XmlDialect {
    paragraph: &["characters"],
    intact: &["script"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
pub const TXML: XmlDialect = XmlDialect {
    paragraph: &["source"],
    intact: &["ut", "skeleton", "revisions"],
    id_attrs: &["id"],
    skip_attr_false: &[],
    inline_shortcuts: true,
};
pub const WORDPRESS: XmlDialect = XmlDialect::new(&["title", "encoded", "description"]);
pub const SCRIBUS: XmlDialect = XmlDialect::new(&["ITEXT"]);
pub const XMLSS: XmlDialect = XmlDialect::new(&["Data"]);
pub const DOCBOOK: XmlDialect = XmlDialect::new(&[
    "title", "subtitle", "para", "simpara", "entry", "term", "glosssee",
]);
pub const VISIO: XmlDialect = XmlDialect {
    paragraph: &["Text"],
    intact: &["DocumentProperties", "DocumentSettings", "Colors"],
    id_attrs: &["ID", "id"],
    skip_attr_false: &[],
    inline_shortcuts: false,
};
