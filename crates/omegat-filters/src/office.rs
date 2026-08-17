use crate::{
    ensure_parent, extract_tags, ExtractedSegment, Filter, FilterContext, FilterError, ParsedFile,
    ProtectedPart, Result,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::{ZipArchive, ZipWriter};

pub struct OpenXmlFilter;
pub struct OpenDocumentFilter;

impl Filter for OpenXmlFilter {
    fn id(&self) -> &'static str {
        "openxml"
    }
    fn name(&self) -> &'static str {
        "Microsoft Office Open XML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.docx", "*.xlsx", "*.pptx"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_zip_xml(path, &["word/document.xml", "xl/sharedStrings.xml", "ppt/slides/"])
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        rewrite_zip(source_path, dest_path, translations)
    }
}

impl Filter for OpenDocumentFilter {
    fn id(&self) -> &'static str {
        "opendoc"
    }
    fn name(&self) -> &'static str {
        "OpenDocument"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.odt", "*.ods", "*.odp"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_zip_xml(path, &["content.xml"])
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        rewrite_zip(source_path, dest_path, translations)
    }
}

fn parse_zip_xml(path: &Path, prefixes: &[&str]) -> Result<ParsedFile> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "office".into(),
        message: e.to_string(),
    })?;
    let mut segments = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "office".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        if !prefixes.iter().any(|p| name.starts_with(p) || name == *p) {
            continue;
        }
        if !name.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        entry.read_to_string(&mut xml)?;
        collect_text_nodes(&xml, &name, &mut segments);
    }
    Ok(ParsedFile {
        segments,
        skeleton: None,
    })
}

fn collect_text_nodes(xml: &str, file: &str, segments: &mut Vec<ExtractedSegment>) {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return;
    };
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let name = node.tag_name().name();
        if !matches!(name, "t" | "p" | "h" | "span") {
            continue;
        }
        let text = node
            .children()
            .filter(|n| n.is_text())
            .map(|n| n.text().unwrap_or(""))
            .collect::<String>();
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let tags = extract_tags(text);
        segments.push(ExtractedSegment {
            id: format!("{}:{}", file, segments.len()),
            source: text.to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: Some(file.to_string()),
            protected_parts: tags
                .into_iter()
                .map(|t| ProtectedPart {
                    text: t,
                    details: "tag".into(),
                })
                .collect(),
        });
    }
}

fn rewrite_zip(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
) -> Result<()> {
    let file = File::open(source_path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "office".into(),
        message: e.to_string(),
    })?;
    ensure_parent(dest_path)?;
    let dest = File::create(dest_path)?;
    let mut writer = ZipWriter::new(dest);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "office".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        let opts = zip::write::FileOptions::default();
        if name.ends_with(".xml") {
            let mut xml = String::new();
            entry.read_to_string(&mut xml)?;
            xml = apply_office_translations(&xml, translations);
            writer.start_file(&name, opts).map_err(|e| FilterError::Parse {
                format: "office".into(),
                message: e.to_string(),
            })?;
            writer.write_all(xml.as_bytes())?;
        } else {
            writer.start_file(&name, opts).map_err(|e| FilterError::Parse {
                format: "office".into(),
                message: e.to_string(),
            })?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            writer.write_all(&buf)?;
        }
    }
    writer.finish().map_err(|e| FilterError::Parse {
        format: "office".into(),
        message: e.to_string(),
    })?;
    Ok(())
}

fn apply_office_translations(xml: &str, translations: &HashMap<String, String>) -> String {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        let mut out = xml.to_string();
        let mut pairs: Vec<(String, String)> = translations
            .iter()
            .filter(|(k, v)| !k.is_empty() && !v.is_empty() && *k != *v)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        for (src, tgt) in pairs {
            if let Some(pos) = out.find(&src) {
                out.replace_range(pos..pos + src.len(), &html_escape::encode_text(&tgt));
            }
        }
        return out;
    };
    let mut replacements = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let name = node.tag_name().name();
        if !matches!(name, "t" | "p" | "h" | "span") {
            continue;
        }
        let text = node
            .children()
            .filter(|n| n.is_text())
            .map(|n| n.text().unwrap_or(""))
            .collect::<String>();
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let Some(tgt) = translations.get(text).filter(|t| !t.is_empty() && *t != text) else {
            continue;
        };
        let range = node.range();
        let slice = &xml[range.start..range.end];
        if let (Some(gt), Some(lt)) = (slice.find('>'), slice.rfind("</")) {
            let start = range.start + gt + 1;
            let end = range.start + lt;
            replacements.push((start, end, html_escape::encode_text(tgt).to_string()));
        }
    }
    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = xml.to_string();
    for (start, end, text) in replacements {
        if start < end && end <= out.len() {
            out.replace_range(start..end, &text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_text_nodes() {
        let xml = r#"<w:t>Hello world</w:t>"#;
        let mut map = HashMap::new();
        map.insert("Hello world".into(), "Bonjour".into());
        assert!(apply_office_translations(xml, &map).contains("Bonjour"));
    }
}
