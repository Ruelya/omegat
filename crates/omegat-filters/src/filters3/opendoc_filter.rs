//! Java `OpenDocFilter`.

use crate::xml_filter::{engine_config, parse_raw_cfg_cancellable, DefaultHooks};
use crate::xml_zip::{
    read_string_cancellable, rewrite_zip_xml_cancellable, run_part_cfg_cancellable,
};
use crate::{Filter, FilterContext, FilterError, ParsedFile, Result};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use zip::ZipArchive;

use super::opendoc_dialect::OpenDocDialect;

const TRANSLATABLE: &[&str] = &["content.xml", "styles.xml", "meta.xml"];

pub struct OpenDocFilter;

fn want(name: &str) -> bool {
    let short = name.rsplit('/').next().unwrap_or(name);
    TRANSLATABLE.contains(&short)
}

fn parse_opendoc(
    path: &Path,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ParsedFile> {
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let dialect = OpenDocDialect::new(&ctx.options);
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "opendoc".into(),
        message: e.to_string(),
    })?;
    let mut segments = Vec::new();
    let mut hooks = DefaultHooks::parse();
    for i in 0..zip.len() {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "opendoc".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        if !want(&name) {
            continue;
        }
        let raw = read_string_cancellable(&mut entry, is_cancelled)?;
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

fn write_opendoc(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    ctx: &FilterContext,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let dialect = OpenDocDialect::new(&ctx.options);
    let translations = translations.clone();
    let cfg = engine_config(ctx);
    let mut hooks = DefaultHooks::write(&translations);
    rewrite_zip_xml_cancellable(
        source_path,
        dest_path,
        want,
        &dialect,
        |name, raw| {
            hooks.enter_part(format!("{name}#"));
            Ok(run_part_cfg_cancellable(
                raw,
                &dialect,
                &mut hooks,
                cfg,
                is_cancelled,
            )?
            .output)
        },
        is_cancelled,
    )
}

impl Filter for OpenDocFilter {
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
        3
    }
    fn file_supported(&self, path: &Path, _ctx: &FilterContext) -> bool {
        let Ok(file) = File::open(path) else {
            return false;
        };
        let Ok(mut zip) = ZipArchive::new(file) else {
            return false;
        };
        for i in 0..zip.len() {
            if let Ok(entry) = zip.by_index(i) {
                if want(entry.name()) {
                    return true;
                }
            }
        }
        false
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        parse_opendoc(path, ctx, &|| false)
    }
    fn parse_cancellable(
        &self,
        path: &Path,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ParsedFile> {
        parse_opendoc(path, ctx, is_cancelled)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        write_opendoc(source_path, dest_path, translations, ctx, &|| false)
    }
    fn write_cancellable(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        write_opendoc(
            source_path,
            dest_path,
            translations,
            ctx,
            is_cancelled,
        )
    }
}
