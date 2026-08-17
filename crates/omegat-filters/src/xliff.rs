use crate::{
    ensure_parent, extract_tags, read_to_string, ExtractedSegment, Filter, FilterContext,
    FilterError, ParsedFile, ProtectedPart, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct Xliff1Filter;
pub struct Xliff2Filter;
pub struct SdlXliffFilter;
pub struct SdlProjectFilter;

impl Filter for Xliff1Filter {
    fn id(&self) -> &'static str {
        "xliff1"
    }
    fn name(&self) -> &'static str {
        "XLIFF 1"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xlf", "*.xliff"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_xliff(&read_to_string(path)?, false)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        write_xliff(source_path, dest_path, translations, "target")
    }
}

impl Filter for Xliff2Filter {
    fn id(&self) -> &'static str {
        "xliff2"
    }
    fn name(&self) -> &'static str {
        "XLIFF 2"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xlf", "*.xliff"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn matches(&self, path: &Path) -> bool {
        if !Xliff1Filter.matches(path) {
            return false;
        }
        read_to_string(path)
            .map(|s| s.contains("urn:oasis:names:tc:xliff:document:2.0") || s.contains("version=\"2."))
            .unwrap_or(false)
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_xliff(&read_to_string(path)?, true)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        write_xliff(source_path, dest_path, translations, "target")
    }
}

impl Filter for SdlXliffFilter {
    fn id(&self) -> &'static str {
        "sdlxliff"
    }
    fn name(&self) -> &'static str {
        "SDL XLIFF"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.sdlxliff"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        Xliff1Filter.parse(path, ctx)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        Xliff1Filter.write(source_path, dest_path, translations, ctx)
    }
}

impl Filter for SdlProjectFilter {
    fn id(&self) -> &'static str {
        "sdlproject"
    }
    fn name(&self) -> &'static str {
        "SDL project"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.sdlproj"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(ParsedFile {
            segments: vec![ExtractedSegment {
                id: "0".into(),
                source: format!("SDL project: {}", path.display()),
                existing_translation: None,
                note: Some("Open the contained .sdlxliff files instead".into()),
                comment: None,
                path: None,
                protected_parts: vec![],
            }],
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        _translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        ensure_parent(dest_path)?;
        std::fs::copy(source_path, dest_path)?;
        Ok(())
    }
}

fn parse_xliff(raw: &str, xliff2: bool) -> Result<ParsedFile> {
    let doc = roxmltree::Document::parse(raw).map_err(|e| FilterError::Parse {
        format: "xliff".into(),
        message: e.to_string(),
    })?;
    let mut segments = Vec::new();
    let source_tag = if xliff2 { "source" } else { "source" };
    let target_tag = "target";
    let unit_tag = if xliff2 { "unit" } else { "trans-unit" };

    for unit in doc.descendants().filter(|n| n.has_tag_name(unit_tag)) {
        let id = unit
            .attribute("id")
            .or_else(|| unit.attribute("resname"))
            .unwrap_or("")
            .to_string();
        let source = unit
            .descendants()
            .find(|n| n.has_tag_name(source_tag))
            .map(|n| inner_text(n))
            .unwrap_or_default();
        if source.trim().is_empty() {
            continue;
        }
        let target = unit
            .descendants()
            .find(|n| n.has_tag_name(target_tag))
            .map(|n| inner_text(n))
            .filter(|s| !s.is_empty());
        let note = unit
            .descendants()
            .find(|n| n.has_tag_name("note"))
            .map(|n| inner_text(n));
        let tags = extract_tags(&source);
        segments.push(ExtractedSegment {
            id: if id.is_empty() {
                segments.len().to_string()
            } else {
                id
            },
            source,
            existing_translation: target,
            note,
            comment: None,
            path: None,
            protected_parts: tags
                .into_iter()
                .map(|t| ProtectedPart {
                    text: t,
                    details: "tag".into(),
                })
                .collect(),
        });
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

fn inner_text(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .map(|n| n.text().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("")
}

fn write_xliff(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    _target_tag: &str,
) -> Result<()> {
    let raw = read_to_string(source_path)?;
    let parsed = parse_xliff(&raw, raw.contains("urn:oasis:names:tc:xliff:document:2.0"))?;
    let mut out = raw;
    for seg in parsed.segments {
        if let Some(t) = translations.get(&seg.id) {
            // Replace first empty or existing target after this source occurrence.
            let escaped = html_escape::encode_text(t).to_string();
            if let Some(pos) = out.find(&seg.source) {
                let rest = &out[pos + seg.source.len()..];
                if let Some(rel) = rest.find("<target") {
                    if let Some(end_rel) = rest[rel..].find("</target>") {
                        let start = pos + seg.source.len() + rel;
                        let end = start + end_rel + "</target>".len();
                        let open_end = out[start..end].find('>').map(|i| start + i + 1).unwrap_or(start);
                        let new_target = format!(
                            "{}{}</target>",
                            &out[start..open_end],
                            escaped
                        );
                        out.replace_range(start..end, &new_target);
                        continue;
                    }
                }
                if let Some(rel) = rest.find("</trans-unit>") {
                    let insert_at = pos + seg.source.len() + rel;
                    let inject = format!("<target>{escaped}</target>");
                    out.insert_str(insert_at, &inject);
                } else if let Some(rel) = rest.find("</unit>") {
                    let insert_at = pos + seg.source.len() + rel;
                    let inject = format!("<target>{escaped}</target>");
                    out.insert_str(insert_at, &inject);
                }
            }
        }
    }
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, out)?;
    Ok(())
}
