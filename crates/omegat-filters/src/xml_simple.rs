//! Linear XML filters (filters3). Extract text from configured element names.

use crate::{
    ensure_parent, extract_tags, read_to_string, ExtractedSegment, Filter, FilterContext,
    FilterError, ParsedFile, ProtectedPart, Result,
};
use std::collections::HashMap;
use std::path::Path;

macro_rules! xml_filter {
    ($ty:ident, $id:expr, $name:expr, $masks:expr, $phase:expr, $tags:expr) => {
        pub struct $ty;
        impl Filter for $ty {
            fn id(&self) -> &'static str {
                $id
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn default_masks(&self) -> &'static [&'static str] {
                $masks
            }
            fn phase(&self) -> u8 {
                $phase
            }
            fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
                parse_xml_tags(&read_to_string(path)?, $tags)
            }
            fn write(
                &self,
                source_path: &Path,
                dest_path: &Path,
                translations: &HashMap<String, String>,
                _ctx: &FilterContext,
            ) -> Result<()> {
                write_xml_tags(source_path, dest_path, translations, $tags)
            }
        }
    };
}

fn parse_xml_tags(raw: &str, tags: &[&str]) -> Result<ParsedFile> {
    let doc = match roxmltree::Document::parse(raw) {
        Ok(d) => d,
        Err(_) => {
            return fallback_tag_scan(raw, tags);
        }
    };
    let mut segments = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let name = node.tag_name().name();
        if !tags.iter().any(|t| *t == name || name.ends_with(t)) {
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
        let id = node
            .attribute("name")
            .or_else(|| node.attribute("id"))
            .or_else(|| node.attribute("key"))
            .unwrap_or("")
            .to_string();
        let tags_in = extract_tags(text);
        segments.push(ExtractedSegment {
            id: if id.is_empty() {
                segments.len().to_string()
            } else {
                id
            },
            source: text.to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: Some(name.to_string()),
            protected_parts: tags_in
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

fn fallback_tag_scan(raw: &str, tags: &[&str]) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    for tag in tags {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        let mut pos = 0usize;
        while let Some(rel_start) = raw[pos..].find(&open) {
            let start = pos + rel_start;
            let after = &raw[start..];
            let Some(gt) = after.find('>') else { break };
            let content_start = start + gt + 1;
            if content_start >= raw.len() {
                break;
            }
            let Some(rel) = raw[content_start..].find(&close) else { break };
            let content = raw[content_start..content_start + rel].trim();
            if !content.is_empty() && !content.contains('<') {
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
            pos = content_start + rel + close.len();
            if pos <= start {
                pos = start + open.len();
            }
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

fn write_xml_tags(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    tags: &[&str],
) -> Result<()> {
    let raw = read_to_string(source_path)?;
    let parsed = parse_xml_tags(&raw, tags)?;
    let mut out = raw;
    for seg in parsed.segments {
        if let Some(t) = translations.get(&seg.id) {
            let escaped = html_escape::encode_text(t).to_string();
            if let Some(pos) = out.find(&seg.source) {
                out.replace_range(pos..pos + seg.source.len(), &escaped);
            }
        }
    }
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, out)?;
    Ok(())
}

xml_filter!(AndroidFilter, "android", "Android Resources", &["*.xml"], 3, &["string"]);
xml_filter!(XhtmlFilter, "xhtml", "XHTML", &["*.xhtml"], 3, &["p", "h1", "h2", "h3", "h4", "li", "td", "th", "title"]);
xml_filter!(PropertiesXmlFilter, "propxml", "Java Properties XML", &["*.xml"], 3, &["entry"]);
xml_filter!(ResxFilter, "resx", "ResX", &["*.resx"], 3, &["value"]);
xml_filter!(WixFilter, "wix", "WiX Localization", &["*.wxl"], 3, &["String"]);
xml_filter!(SvgFilter, "svg", "SVG", &["*.svg"], 3, &["text", "tspan", "title", "desc"]);
xml_filter!(HelpAndManualFilter, "helpandmanual", "Help & Manual", &["*.xml"], 3, &["caption", "text"]);
xml_filter!(SchematronFilter, "schematron", "Schematron", &["*.sch"], 3, &["assert", "report"]);
xml_filter!(RelaxNgFilter, "relaxng", "RELAX NG", &["*.rng"], 3, &["documentation"]);
xml_filter!(CamtasiaFilter, "camtasia", "Camtasia for Windows", &["*.camproj"], 3, &["caption", "title"]);
xml_filter!(Typo3Filter, "typo3", "Typo3 LocManager", &["*.xml"], 3, &["label", "value"]);
xml_filter!(L10nMgrFilter, "l10nmgr", "Typo3 l10nmgr", &["*.xml"], 3, &["data"]);
xml_filter!(InfixFilter, "infix", "Infix", &["*.xml"], 3, &["text"]);
xml_filter!(FlashFilter, "flash", "Flash XML Export", &["*.xml"], 3, &["string", "text"]);
xml_filter!(TxmlFilter, "txml", "Wordfast TXML", &["*.txml"], 3, &["seg"]);
xml_filter!(WordpressFilter, "wordpress", "Wordpress XML export", &["*.xml"], 3, &["title", "content:encoded", "excerpt:encoded", "description"]);
xml_filter!(ScribusFilter, "scribus", "Scribus", &["*.sla"], 3, &["ITEXT"]);
xml_filter!(XmlSpreadsheetFilter, "xmlss", "XML Spreadsheet 2003", &["*.xml"], 3, &["Data"]);
xml_filter!(DocBookFilter, "docbook", "DocBook", &["*.xml", "*.dbk"], 4, &["para", "title", "simpara", "entry"]);
xml_filter!(VisioFilter, "visio", "Visio", &["*.vdx", "*.vsdx"], 4, &["text", "Text"]);

impl AndroidFilter {
    #[allow(dead_code)]
    fn _err() -> FilterError {
        FilterError::Unsupported("android".into())
    }
}
