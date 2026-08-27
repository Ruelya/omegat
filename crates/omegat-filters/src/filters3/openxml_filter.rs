//! Java `OpenXMLFilter` (filters3 ZIP + dialect).

use crate::xml_filter::{engine_config, parse_raw_cfg, DefaultHooks};
use crate::xml_zip::rewrite_zip_xml;
use crate::{Filter, FilterContext, FilterError, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use super::openxml_dialect::OpenXmlDialect;

pub struct OpenXmlFilter;

fn translatable_re(options: &HashMap<String, String>) -> Regex {
    let mut sb = String::from(r"(document\d?\.xml)");
    if flag(options, "translateComments", true) {
        sb.push_str(r"|(comments\.xml)");
    }
    if flag(options, "translateFootnotes", true) {
        sb.push_str(r"|(footnotes\.xml)");
    }
    if flag(options, "translateEndnotes", true) {
        sb.push_str(r"|(endnotes\.xml)");
    }
    if flag(options, "translateHeaders", true) {
        sb.push_str(r"|(header\d+\.xml)");
    }
    if flag(options, "translateFooters", true) {
        sb.push_str(r"|(footer\d+\.xml)");
    }
    if flag(options, "documentProperties", false) {
        sb.push_str(r"|(core\.xml)");
    }
    sb.push_str(r"|(sharedStrings\.xml)");
    if flag(options, "translateExcelComments", true) {
        sb.push_str(r"|(comments\d+\.xml)");
    }
    sb.push_str(r"|(slide\d+\.xml)");
    if flag(options, "translateSlideMasters", false) {
        sb.push_str(r"|(slideMaster\d+\.xml)");
    }
    if flag(options, "translateSlideLayouts", false) {
        sb.push_str(r"|(slideLayout\d+\.xml)");
    }
    if flag(options, "translateSlideComments", true) {
        sb.push_str(r"|(notesSlide\d+\.xml)");
    }
    if flag(options, "translateDiagrams", false) {
        sb.push_str(r"|(data\d+\.xml)");
    }
    if flag(options, "translateCharts", false) {
        sb.push_str(r"|(chart\d+\.xml)");
    }
    if flag(options, "translateDrawings", false) {
        sb.push_str(r"|(drawing\d+\.xml)");
    }
    if flag(options, "translateSheetNames", false) {
        sb.push_str(r"|(workbook\.xml)");
    }
    if flag(options, "translateSlideLinks", false) {
        sb.push_str(r"|(\w+\d*\.xml\.rels)");
    }
    sb.push_str(r"|(page\d+\.xml)");
    Regex::new(&format!("^({sb})$")).unwrap()
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

fn short_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

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
        3
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = OpenXmlDialect::new(&ctx.options);
        let re = translatable_re(&ctx.options);
        let file = File::open(path)?;
        let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
            format: "openxml".into(),
            message: e.to_string(),
        })?;
        let mut segments = Vec::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
                format: "openxml".into(),
                message: e.to_string(),
            })?;
            let name = entry.name().to_string();
            if !re.is_match(short_name(&name)) {
                continue;
            }
            let mut raw = String::new();
            if entry.read_to_string(&mut raw).is_err() {
                continue;
            }
            let mut hooks = DefaultHooks::parse();
            if let Ok(parsed) = parse_raw_cfg(&raw, &dialect, &mut hooks, engine_config(ctx)) {
                segments.extend(parsed.segments);
            }
        }
        Ok(ParsedFile {
            segments,
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = OpenXmlDialect::new(&ctx.options);
        let re = translatable_re(&ctx.options);
        let translations = translations.clone();
        rewrite_zip_xml(
            source_path,
            dest_path,
            |n| re.is_match(short_name(n)),
            &dialect,
            |_name, raw| {
                let mut hooks = DefaultHooks::write(&translations);
                Ok(crate::xml_zip::run_part_cfg(raw, &dialect, &mut hooks, engine_config(ctx))?.output)
            },
        )
    }
}
