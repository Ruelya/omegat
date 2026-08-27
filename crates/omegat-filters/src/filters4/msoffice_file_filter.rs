//! Java `org.omegat.filters4.xml.openxml.MsOfficeFileFilter`.

use super::abstract_zip::{
    parse_zip_parts_cancellable, short_name, write_zip_parts_cancellable,
};
use super::openxml_filter::{
    parse_openxml_part_cancellable, write_openxml_part_cancellable,
};
use crate::{Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

pub struct MsOfficeFileFilter;

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

/// Java `defineDOCUMENTSOptions`.
pub fn documents_pattern(options: &HashMap<String, String>) -> String {
    let mut d = String::from("(document\\d?\\.xml)");
    if flag(options, "translateComments", true) {
        d.push_str("|(comments\\.xml)");
    }
    if flag(options, "translateFootnotes", true) {
        d.push_str("|(footnotes\\.xml)");
    }
    if flag(options, "translateEndnotes", true) {
        d.push_str("|(endnotes\\.xml)");
    }
    if flag(options, "translateHeaders", true) {
        d.push_str("|(header\\d+\\.xml)");
    }
    if flag(options, "translateFooters", true) {
        d.push_str("|(footer\\d+\\.xml)");
    }
    d.push_str("|(sharedStrings\\.xml)");
    if flag(options, "translateExcelComments", true) {
        d.push_str("|(comments\\d+\\.xml)");
    }
    d.push_str("|(slide\\d+\\.xml)");
    if flag(options, "translateSlideMasters", false) {
        d.push_str("|(slideMaster\\d+\\.xml)");
    }
    if flag(options, "translateSlideLayouts", false) {
        d.push_str("|(slideLayout\\d+\\.xml)");
    }
    if flag(options, "translateSlideComments", true) {
        d.push_str("|(notesSlide\\d+\\.xml)");
    }
    if flag(options, "translateDiagrams", false) {
        d.push_str("|(data\\d+\\.xml)");
    }
    if flag(options, "translateCharts", false) {
        d.push_str("|(chart\\d+\\.xml)");
    }
    if flag(options, "translateDrawings", false) {
        d.push_str("|(drawing\\d+\\.xml)");
    }
    if flag(options, "translateSheetNames", false) {
        d.push_str("|(workbook\\.xml)");
    }
    d.push_str("|(page\\d+\\.xml)");
    d
}

fn translatable_re(options: &HashMap<String, String>) -> Regex {
    let d = documents_pattern(options);
    Regex::new(&format!("^({d})$")).unwrap()
}

fn accept_internal(name: &str) -> bool {
    name.ends_with("document.xml")
        || name.ends_with("document2.xml")
        || name.ends_with("sharedStrings.xml")
        || name.ends_with("slide1.xml")
        || name.ends_with("page1.xml")
}

fn must_translate(name: &str, write_mode: bool, options: &HashMap<String, String>) -> bool {
    if write_mode && name.contains("word") && name.contains("styles") {
        return true;
    }
    translatable_re(options).is_match(short_name(name))
}

fn must_delete(name: &str, options: &HashMap<String, String>) -> bool {
    if name.ends_with("comments.xml") {
        return !documents_pattern(options).contains("comments");
    }
    false
}

fn digits_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)\.xml").unwrap())
}

/// Java `getEntryComparator`.
pub fn cmp_entries(a: &str, b: &str, documents: &str) -> std::cmp::Ordering {
    let words1: Vec<&str> = a.split(|c: char| c.is_ascii_digit()).collect();
    // Java splits on `\\d+\\.` — keep a simpler numeric-aware compare when prefixes match.
    let stem1 = a.split(|c: char| c.is_ascii_digit()).next().unwrap_or(a);
    let stem2 = b.split(|c: char| c.is_ascii_digit()).next().unwrap_or(b);
    let has_digits = digits_re().is_match(a) && digits_re().is_match(b);
    if has_digits && stem1 == stem2 {
        let n1 = digits_re()
            .captures(a)
            .and_then(|c| c[1].parse::<i32>().ok())
            .unwrap_or(0);
        let n2 = digits_re()
            .captures(b)
            .and_then(|c| c[1].parse::<i32>().ok())
            .unwrap_or(0);
        return n1.cmp(&n2);
    }
    let mut s1 = short_name(words1.first().copied().unwrap_or(a)).to_string();
    let words2: Vec<&str> = b.split(|c: char| c.is_ascii_digit()).collect();
    let mut s2 = short_name(words2.first().copied().unwrap_or(b)).to_string();
    if s1.contains("sharedStrings") || s2.contains("sharedStrings") {
        return if s2.contains("sharedStrings") {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        };
    }
    if let Some(i) = s1.rfind('.') {
        if s1.ends_with(".xml") {
            s1.truncate(i);
        }
    }
    if let Some(i) = s2.rfind('.') {
        if s2.ends_with(".xml") {
            s2.truncate(i);
        }
    }
    let i1 = documents.find(&s1).unwrap_or(usize::MAX);
    let i2 = documents.find(&s2).unwrap_or(usize::MAX);
    i1.cmp(&i2).then_with(|| a.cmp(b))
}

fn parse_msoffice(
    path: &Path,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ParsedFile> {
    let docs = documents_pattern(&ctx.options);
    let options = ctx.options.clone();
    let ctx = ctx.clone();
    let with_comments = docs.contains("comments");
    let segments = parse_zip_parts_cancellable(
        path,
        accept_internal,
        |n| must_translate(n, false, &options),
        |_name, raw| {
            parse_openxml_part_cancellable(raw, &ctx, with_comments, is_cancelled)
        },
        Some(|a: &str, b: &str| cmp_entries(a, b, &docs)),
        is_cancelled,
    )?;
    Ok(ParsedFile {
        segments,
        skeleton: None,
    })
}

fn write_msoffice(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let docs = documents_pattern(&ctx.options);
    let options = ctx.options.clone();
    let ctx = ctx.clone();
    let with_comments = docs.contains("comments");
    let translations = translations.clone();
    write_zip_parts_cancellable(
        source_path,
        dest_path,
        |n| must_translate(n, true, &options),
        |n| must_delete(n, &options),
        |_name, raw| {
            write_openxml_part_cancellable(
                raw,
                &ctx,
                with_comments,
                &translations,
                is_cancelled,
            )
        },
        is_cancelled,
    )
}

impl Filter for MsOfficeFileFilter {
    fn id(&self) -> &'static str {
        "msoffice"
    }
    fn name(&self) -> &'static str {
        "Microsoft Office Open XML (filters4)"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.doc?", "*.dotx", "*.xls?", "*.ppt?", "*.vsdx"]
    }
    fn phase(&self) -> u8 {
        4
    }
    /// ZIP with a translatable Office part, or the inner `OpenXmlFilter`
    /// document (`document.xml`) used by `OpenXmlFilterTest`.
    fn file_supported(&self, path: &Path, _ctx: &FilterContext) -> bool {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if accept_internal(&name) {
            return true;
        }
        if let Ok(file) = std::fs::File::open(path) {
            if let Ok(mut zip) = zip::ZipArchive::new(file) {
                for i in 0..zip.len() {
                    if let Ok(entry) = zip.by_index(i) {
                        if accept_internal(entry.name()) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        parse_msoffice(path, ctx, &|| false)
    }
    fn parse_cancellable(
        &self,
        path: &Path,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ParsedFile> {
        parse_msoffice(path, ctx, is_cancelled)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        write_msoffice(source_path, dest_path, translations, ctx, &|| false)
    }
    fn write_cancellable(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        write_msoffice(
            source_path,
            dest_path,
            translations,
            ctx,
            is_cancelled,
        )
    }
}
