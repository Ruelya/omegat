//! File format filters. Each filter extracts translatable segments and can
//! write translations back into a target file.

mod csv;
mod dialect;
mod html;
mod json;
mod markdown;
mod misc;
mod office;
mod pdf;
mod po;
mod properties;
mod subtitle;
mod text;
mod xml_simple;
mod xliff;
mod yaml;

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse {format}: {message}")]
    Parse { format: String, message: String },
    #[error("unsupported file: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, FilterError>;

#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    pub source_lang: String,
    pub target_lang: String,
    pub remove_tags: bool,
    /// Java `processOptions` map (e.g. `segmentOn`, `skipHeader`).
    pub options: HashMap<String, String>,
}

impl FilterContext {
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn option_flag(&self, key: &str) -> bool {
        matches!(
            self.option(key).map(|s| s.to_ascii_lowercase()).as_deref(),
            Some("true") | Some("yes") | Some("1")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedPart {
    pub text: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSegment {
    pub id: String,
    pub source: String,
    pub existing_translation: Option<String>,
    pub note: Option<String>,
    pub comment: Option<String>,
    pub path: Option<String>,
    pub protected_parts: Vec<ProtectedPart>,
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub segments: Vec<ExtractedSegment>,
    /// Skeleton with `\u{0000}N\u{0000}` placeholders, if the filter uses it.
    pub skeleton: Option<String>,
}

pub trait Filter: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn default_masks(&self) -> &'static [&'static str];
    fn phase(&self) -> u8 {
        1
    }
    fn matches(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        self.default_masks().iter().any(|mask| {
            let ext = mask.trim_start_matches("*").to_ascii_lowercase();
            name.ends_with(&ext)
        })
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile>;
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()>;
}

pub struct FilterInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub masks: &'static [&'static str],
    pub phase: u8,
}

pub struct FilterRegistry {
    filters: Vec<Box<dyn Filter>>,
}

impl Default for FilterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterRegistry {
    pub fn new() -> Self {
        let filters: Vec<Box<dyn Filter>> = vec![
            Box::new(text::TextFilter),
            Box::new(html::HtmlFilter),
            Box::new(po::PoFilter),
            Box::new(xliff::Xliff1Filter),
            Box::new(json::JsonFilter),
            Box::new(properties::PropertiesFilter),
            Box::new(csv::CsvFilter),
            Box::new(yaml::YamlFilter),
            Box::new(subtitle::SrtFilter),
            Box::new(markdown::MarkdownFilter),
            // P3
            Box::new(misc::LatexFilter),
            Box::new(misc::RcFilter),
            Box::new(misc::MoodlePhpFilter),
            Box::new(misc::MozillaDtdFilter),
            Box::new(misc::MozillaLangFilter),
            Box::new(misc::MozillaFtlFilter),
            Box::new(misc::HhcFilter),
            Box::new(misc::IniFilter),
            Box::new(misc::DokuWikiFilter),
            Box::new(misc::MagentoFilter),
            Box::new(misc::IliasFilter),
            Box::new(subtitle::SbvFilter),
            Box::new(subtitle::WebVttFilter),
            Box::new(misc::XtagFilter),
            Box::new(xml_simple::AndroidFilter),
            Box::new(xml_simple::XhtmlFilter),
            Box::new(xml_simple::PropertiesXmlFilter),
            Box::new(xml_simple::ResxFilter),
            Box::new(xml_simple::WixFilter),
            Box::new(xml_simple::SvgFilter),
            Box::new(xml_simple::HelpAndManualFilter),
            Box::new(xml_simple::SchematronFilter),
            Box::new(xml_simple::RelaxNgFilter),
            Box::new(xml_simple::CamtasiaFilter),
            Box::new(xml_simple::Typo3Filter),
            Box::new(xml_simple::L10nMgrFilter),
            Box::new(xml_simple::InfixFilter),
            Box::new(xml_simple::FlashFilter),
            Box::new(xml_simple::TxmlFilter),
            Box::new(xml_simple::WordpressFilter),
            Box::new(xml_simple::ScribusFilter),
            Box::new(xml_simple::XmlSpreadsheetFilter),
            // P4
            Box::new(office::OpenDocumentFilter),
            Box::new(office::OpenXmlFilter),
            Box::new(xml_simple::DocBookFilter),
            Box::new(xml_simple::VisioFilter),
            Box::new(xliff::Xliff2Filter),
            Box::new(xliff::SdlXliffFilter),
            Box::new(xliff::SdlProjectFilter),
            Box::new(pdf::PdfFilter),
        ];
        Self { filters }
    }

    pub fn all(&self) -> &[Box<dyn Filter>] {
        &self.filters
    }

    pub fn info(&self) -> Vec<FilterInfo> {
        self.filters
            .iter()
            .map(|f| FilterInfo {
                id: f.id(),
                name: f.name(),
                masks: f.default_masks(),
                phase: f.phase(),
            })
            .collect()
    }

    pub fn for_path(&self, path: &Path) -> Option<&dyn Filter> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let by_ext: Option<&str> = match ext.as_str() {
            "txt" => Some("text"),
            "html" | "htm" => Some("html"),
            "po" | "pot" => Some("po"),
            "json" => Some("json"),
            "properties" => Some("properties"),
            "csv" => Some("csv"),
            "tsv" => Some("csv"),
            "yaml" | "yml" => Some("yaml"),
            "srt" => Some("srt"),
            "sbv" => Some("sbv"),
            "vtt" => Some("webvtt"),
            "md" | "markdown" => Some("markdown"),
            "tex" => Some("latex"),
            "rc" => Some("rc"),
            "php" => Some("moodlephp"),
            "dtd" => Some("mozdtd"),
            "ftl" => Some("mozftl"),
            "hhc" => Some("hhc"),
            "ini" => Some("ini"),
            "xtg" => Some("xtag"),
            "resx" => Some("resx"),
            "wxl" => Some("wix"),
            "svg" => Some("svg"),
            "sch" => Some("schematron"),
            "rng" => Some("relaxng"),
            "camproj" => Some("camtasia"),
            "txml" => Some("txml"),
            "sla" => Some("scribus"),
            "docx" | "xlsx" | "pptx" => Some("openxml"),
            "odt" | "ods" | "odp" => Some("opendoc"),
            "pdf" => Some("pdf"),
            "sdlxliff" => Some("sdlxliff"),
            "sdlproj" => Some("sdlproject"),
            "xlf" | "xliff" => {
                if let Ok(s) = read_to_string(path) {
                    if s.contains("urn:oasis:names:tc:xliff:document:2.0") || s.contains("version=\"2.")
                    {
                        Some("xliff2")
                    } else {
                        Some("xliff1")
                    }
                } else {
                    Some("xliff1")
                }
            }
            "xml" | "xhtml" => return self.sniff_xml(path),
            "lang" => Some("mozlang"),
            _ => None,
        };
        if let Some(id) = by_ext {
            if let Some(f) = self.by_id(id) {
                return Some(f);
            }
        }
        self.filters
            .iter()
            .find(|f| f.matches(path))
            .map(|f| f.as_ref())
    }

    fn sniff_xml(&self, path: &Path) -> Option<&dyn Filter> {
        let raw = read_to_string(path).ok()?;
        let id = if raw.contains("<string ") && raw.contains("resources") {
            "android"
        } else if raw.contains("<entry") && raw.contains("properties") {
            "propxml"
        } else if raw.contains("DocBook") || raw.contains("<para") && raw.contains("<book") {
            "docbook"
        } else if raw.contains("http://www.w3.org/1999/xhtml") || path.extension().and_then(|e| e.to_str()) == Some("xhtml") {
            "xhtml"
        } else if raw.contains("WordPress") || raw.contains("content:encoded") {
            "wordpress"
        } else if raw.contains("Spreadsheet") && raw.contains("<Data") {
            "xmlss"
        } else if raw.contains("HelpAndManual") || raw.contains("<topic") {
            "helpandmanual"
        } else if raw.contains("<string") && raw.contains("name=") && raw.contains("resources") {
            "android"
        } else if raw.contains("<para") || raw.contains("<chapter") {
            "docbook"
        } else {
            return None;
        };
        self.by_id(id)
    }

    pub fn by_id(&self, id: &str) -> Option<&dyn Filter> {
        self.filters
            .iter()
            .find(|f| f.id() == id)
            .map(|f| f.as_ref())
    }
}

pub fn placeholder(index: usize) -> String {
    format!("\u{0000}{index}\u{0000}")
}

pub fn apply_skeleton(skeleton: &str, translations: &HashMap<String, String>) -> String {
    apply_skeleton_with_originals(skeleton, translations, &[])
}

/// Replace placeholders. Missing translations keep `originals[i]` (empty-write preserve).
pub fn apply_skeleton_with_originals(
    skeleton: &str,
    translations: &HashMap<String, String>,
    originals: &[String],
) -> String {
    let mut out = skeleton.to_string();
    let mut i = 0usize;
    loop {
        let token = placeholder(i);
        if !out.contains(&token) {
            break;
        }
        let id = i.to_string();
        let repl = translations
            .get(&id)
            .cloned()
            .or_else(|| translations.get(&format!("seg-{i}")).cloned())
            .or_else(|| originals.get(i).cloned())
            .unwrap_or_default();
        out = out.replace(&token, &repl);
        i += 1;
    }
    out
}

/// Overlay translations onto a source-keyed / id-keyed map, keeping originals when absent.
pub fn merge_translations(
    segments: &[ExtractedSegment],
    translations: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (i, seg) in segments.iter().enumerate() {
        let t = translations
            .get(&seg.id)
            .cloned()
            .or_else(|| translations.get(&seg.source).cloned())
            .or_else(|| translations.get(&i.to_string()).cloned())
            .unwrap_or_else(|| seg.source.clone());
        out.insert(seg.id.clone(), t.clone());
        out.insert(i.to_string(), t.clone());
        if !seg.source.is_empty() {
            out.insert(seg.source.clone(), t);
        }
    }
    out
}

pub fn read_to_string(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let (cow, _, _) = encoding_rs::UTF_8.decode(&bytes);
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(cow.trim_start_matches('\u{feff}').to_string());
    }
    // Try UTF-8 first; fall back to windows-1252 for legacy files.
    if std::str::from_utf8(&bytes).is_ok() {
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
    Ok(cow.into_owned())
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn extract_tags(text: &str) -> Vec<String> {
    let re = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"<[^>]+>|\{[0-9]+\}|%\d+\$[sd]|%s|%d").unwrap()
    });
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn text_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("a.txt");
        let mut f = std::fs::File::create(&src).unwrap();
        writeln!(f, "Hello world.\n\nSecond paragraph.").unwrap();
        let reg = FilterRegistry::new();
        let filter = reg.for_path(&src).unwrap();
        let parsed = filter.parse(&src, &FilterContext::default()).unwrap();
        assert!(parsed.segments.len() >= 2);
        let mut tr = HashMap::new();
        for (i, seg) in parsed.segments.iter().enumerate() {
            tr.insert(seg.id.clone(), format!("T{i}"));
        }
        let dest = dir.path().join("out.txt");
        filter
            .write(&src, &dest, &tr, &FilterContext::default())
            .unwrap();
        let out = std::fs::read_to_string(&dest).unwrap();
        assert!(out.contains("T0"));
    }

    #[test]
    fn registry_covers_core_ids() {
        let reg = FilterRegistry::new();
        for id in ["text", "html", "po", "xliff1", "json", "properties", "srt"] {
            assert!(reg.by_id(id).is_some(), "missing {id}");
        }
        assert!(reg.all().len() >= 40);
    }
}
