//! ZIP wrappers used by filters3 OpenDoc / OpenXML.

use crate::xml_dialect::XmlDialect;
use crate::xml_engine::{process_xml, EngineConfig, FilterHooks, ProcessResult};
use crate::{ensure_parent, ExtractedSegment, FilterError, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::{ZipArchive, ZipWriter};

pub fn parse_zip_xml(
    path: &Path,
    want: impl Fn(&str) -> bool,
    dialect: &dyn XmlDialect,
    mut make_hooks: impl FnMut() -> Box<dyn FilterHooks>,
) -> Result<(Vec<ExtractedSegment>, Option<String>)> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    let mut segments = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "zip".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        let short = name.rsplit('/').next().unwrap_or(&name).to_string();
        if !want(&short) && !want(&name) {
            continue;
        }
        let mut raw = String::new();
        if entry.read_to_string(&mut raw).is_err() {
            continue;
        }
        let mut hooks = make_hooks();
        let _ = process_xml(&raw, dialect, hooks.as_mut(), EngineConfig::default());
        // DefaultHooks is not object-safe for taking segments; callers use typed hooks.
        let _ = hooks;
        let _ = segments;
    }
    let _ = dialect;
    Ok((segments, None))
}

pub fn rewrite_zip_xml(
    source: &Path,
    dest: &Path,
    want: impl Fn(&str) -> bool,
    dialect: &dyn XmlDialect,
    mut process_part: impl FnMut(&str, &str) -> Result<String>,
) -> Result<()> {
    let file = File::open(source)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    ensure_parent(dest)?;
    let out = File::create(dest)?;
    let mut writer = ZipWriter::new(out);
    let opts = zip::write::FileOptions::default();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| FilterError::Parse {
            format: "zip".into(),
            message: e.to_string(),
        })?;
        let name = entry.name().to_string();
        let short = name.rsplit('/').next().unwrap_or(&name).to_string();
        writer
            .start_file(name.clone(), opts)
            .map_err(|e| FilterError::Parse {
                format: "zip".into(),
                message: e.to_string(),
            })?;
        if want(&short) || want(&name) {
            let mut raw = String::new();
            entry.read_to_string(&mut raw)?;
            let rewritten = process_part(&name, &raw)?;
            writer.write_all(rewritten.as_bytes())?;
        } else {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            writer.write_all(&buf)?;
        }
    }
    writer.finish().map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    let _ = dialect;
    Ok(())
}

pub fn run_part(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
) -> Result<ProcessResult> {
    process_xml(raw, dialect, hooks, EngineConfig::default()).map_err(|e| FilterError::Parse {
        format: "xml".into(),
        message: e,
    })
}
