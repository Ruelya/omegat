//! Java `org.omegat.filters4.xml.xliff.SdlProject`.

use super::abstract_xml::process_xml_string_ex;
use super::stax::XmlDeclStyle;
use super::abstract_zip::{parse_zip_parts, write_zip_parts};
use super::xliff1_filter::Xliff1Proc;
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct SdlProjectFilter;

fn target_prefix(ctx: &FilterContext) -> String {
    if ctx.target_lang.is_empty() {
        "be".into()
    } else {
        ctx.target_lang.clone()
    }
}

fn accept(name: &str) -> bool {
    name.ends_with(".sdlxliff")
}

fn translate(name: &str, ctx: &FilterContext) -> bool {
    name.starts_with(&target_prefix(ctx)) && name.ends_with(".sdlxliff")
}

fn parse_inner(raw: &str) -> Result<Vec<crate::ExtractedSegment>> {
    let mut proc = Xliff1Proc::new();
    proc.standard_state = false;
    proc.event_on_cmt_defs = true;
    let (segments, _) = process_xml_string_ex(raw, &mut proc, false, XmlDeclStyle::AbstractXml)?;
    Ok(segments)
}

fn write_inner(raw: &str, translations: &HashMap<String, String>) -> Result<String> {
    let mut proc = Xliff1Proc::with_translations(translations);
    proc.standard_state = false;
    proc.event_on_cmt_defs = true;
    proc.fill_missing_with_source = true;
    let (_, text) = process_xml_string_ex(raw, &mut proc, true, XmlDeclStyle::AbstractXml)?;
    Ok(text)
}

impl Filter for SdlProjectFilter {
    fn id(&self) -> &'static str {
        "sdlproject"
    }
    fn name(&self) -> &'static str {
        "SDL project"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.sdlppx"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let ctx = ctx.clone();
        let segments = parse_zip_parts(
            path,
            accept,
            |n| translate(n, &ctx),
            |_n, raw| parse_inner(raw),
            Some(|a: &str, b: &str| a.cmp(b)),
        )?;
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
        let ctx = ctx.clone();
        let translations = translations.clone();
        write_zip_parts(
            source_path,
            dest_path,
            |n| translate(n, &ctx),
            |_| false,
            |_n, raw| write_inner(raw, &translations),
        )
    }
}
