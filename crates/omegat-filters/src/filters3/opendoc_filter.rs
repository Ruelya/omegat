//! Java `OpenDocFilter`.

use crate::xml_filter::{engine_config, parse_raw_cfg, DefaultHooks};
use crate::xml_zip::rewrite_zip_xml;
use crate::{Filter, FilterContext, FilterError, ParsedFile, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use super::opendoc_dialect::OpenDocDialect;

const TRANSLATABLE: &[&str] = &["content.xml", "styles.xml", "meta.xml"];

pub struct OpenDocFilter;

fn want(name: &str) -> bool {
    let short = name.rsplit('/').next().unwrap_or(name);
    TRANSLATABLE.contains(&short)
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
        let dialect = OpenDocDialect::new(&ctx.options);
        let file = File::open(path)?;
        let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
            format: "opendoc".into(),
            message: e.to_string(),
        })?;
        let mut segments = Vec::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
                format: "opendoc".into(),
                message: e.to_string(),
            })?;
            let name = entry.name().to_string();
            if !want(&name) {
                continue;
            }
            let mut raw = String::new();
            if entry.read_to_string(&mut raw).is_err() {
                continue;
            }
            let mut hooks = DefaultHooks::parse();
            let parsed = parse_raw_cfg(&raw, &dialect, &mut hooks, engine_config(ctx))?;
            segments.extend(parsed.segments);
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
        let dialect = OpenDocDialect::new(&ctx.options);
        let translations = translations.clone();
        let cfg = engine_config(ctx);
        rewrite_zip_xml(source_path, dest_path, want, &dialect, |_name, raw| {
            let mut hooks = DefaultHooks::write(&translations);
            Ok(crate::xml_zip::run_part_cfg(raw, &dialect, &mut hooks, cfg)?.output)
        })
    }
}
