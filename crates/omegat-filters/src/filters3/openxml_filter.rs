//! Java `OpenXMLFilter` (filters3 ZIP + dialect).

use crate::xml_filter::{engine_config, parse_raw_cfg_cancellable, DefaultHooks};
use crate::xml_zip::{
    read_string_cancellable, rewrite_zip_xml_cancellable, run_part_cfg_cancellable,
};
use crate::{Filter, FilterContext, FilterError, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
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

fn numbered_part(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?
        .strip_suffix(".xml")?
        .parse()
        .ok()
}

fn part_sort_key(path: &str) -> (usize, u32, &str) {
    let name = short_name(path);
    let (rank, number) = if name == "sharedStrings.xml" {
        // Java's comparator special-cases this part ahead of every other
        // family so Excel comments are visited only after their shared text.
        (0, 0)
    } else if name == "document.xml" {
        (1, 0)
    } else if let Some(number) = numbered_part(name, "document") {
        (1, number)
    } else if name == "comments.xml" {
        (2, 0)
    } else if name == "footnotes.xml" {
        (3, 0)
    } else if name == "endnotes.xml" {
        (4, 0)
    } else if let Some(number) = numbered_part(name, "header") {
        (5, number)
    } else if let Some(number) = numbered_part(name, "footer") {
        (6, number)
    } else if name == "core.xml" {
        (7, 0)
    } else if let Some(number) = numbered_part(name, "comments") {
        (8, number)
    } else if let Some(number) = numbered_part(name, "slide") {
        (9, number)
    } else if let Some(number) = numbered_part(name, "slideMaster") {
        (10, number)
    } else if let Some(number) = numbered_part(name, "slideLayout") {
        (11, number)
    } else if let Some(number) = numbered_part(name, "notesSlide") {
        (12, number)
    } else if let Some(number) = numbered_part(name, "data") {
        (13, number)
    } else if let Some(number) = numbered_part(name, "chart") {
        (14, number)
    } else if let Some(number) = numbered_part(name, "drawing") {
        (15, number)
    } else if name == "workbook.xml" {
        (16, 0)
    } else if let Some(number) = numbered_part(name, "page") {
        (18, number)
    } else {
        // External relationship parts appear immediately before Visio pages in
        // Java's configured document order.
        (17, 0)
    };
    (rank, number, path)
}

fn parse_openxml(
    path: &Path,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ParsedFile> {
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let dialect = OpenXmlDialect::new(&ctx.options);
    let re = translatable_re(&ctx.options);
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "openxml".into(),
        message: e.to_string(),
    })?;
    let mut parts = Vec::new();
    for i in 0..zip.len() {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "openxml".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        if !re.is_match(short_name(&name)) {
            continue;
        }
        let raw = read_string_cancellable(&mut entry, is_cancelled)?;
        parts.push((name, raw));
    }
    parts.sort_by(|(left, _), (right, _)| part_sort_key(left).cmp(&part_sort_key(right)));

    let mut segments = Vec::new();
    let mut hooks = DefaultHooks::parse();
    for (name, raw) in parts {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        hooks.enter_part(format!("{name}#"));
        let parsed = parse_raw_cfg_cancellable(
            &raw,
            &dialect,
            &mut hooks,
            engine_config(ctx),
            is_cancelled,
        )?;
        segments.extend(parsed.segments);
    }
    Ok(ParsedFile {
        segments,
        skeleton: None,
    })
}

fn write_openxml(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let dialect = OpenXmlDialect::new(&ctx.options);
    let re = translatable_re(&ctx.options);
    let translations = translations.clone();
    let mut hooks = DefaultHooks::write(&translations);
    rewrite_zip_xml_cancellable(
        source_path,
        dest_path,
        |n| re.is_match(short_name(n)),
        &dialect,
        |name, raw| {
            hooks.enter_part(format!("{name}#"));
            Ok(run_part_cfg_cancellable(
                raw,
                &dialect,
                &mut hooks,
                engine_config(ctx),
                is_cancelled,
            )?
            .output)
        },
        is_cancelled,
    )
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
        parse_openxml(path, ctx, &|| false)
    }
    fn parse_cancellable(
        &self,
        path: &Path,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ParsedFile> {
        parse_openxml(path, ctx, is_cancelled)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        write_openxml(source_path, dest_path, translations, ctx, &|| false)
    }
    fn write_cancellable(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        write_openxml(
            source_path,
            dest_path,
            translations,
            ctx,
            is_cancelled,
        )
    }
}
