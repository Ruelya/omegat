//! ZIP wrappers used by filters3 OpenDoc / OpenXML.

use crate::xml_dialect::XmlDialect;
use crate::xml_engine::{
    process_xml, process_xml_cancellable, EngineConfig, FilterHooks, ProcessResult,
};
use crate::{ensure_parent, ExtractedSegment, FilterError, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{ZipArchive, ZipWriter};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryOutput(PathBuf);

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temporary_output(dest: &Path) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    dest.with_file_name(format!(
        ".{name}.omegat-{}-{sequence}.tmp",
        std::process::id()
    ))
}

pub(crate) fn read_string_cancellable(
    reader: &mut dyn Read,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    String::from_utf8(bytes).map_err(|error| {
        FilterError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error,
        ))
    })
}

fn copy_cancellable(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        writer.write_all(&chunk[..count])?;
    }
    if is_cancelled() {
        Err(FilterError::Cancelled)
    } else {
        Ok(())
    }
}

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
    rewrite_zip_xml_cancellable(
        source,
        dest,
        want,
        dialect,
        process_part,
        &|| false,
    )
}

pub fn rewrite_zip_xml_cancellable(
    source: &Path,
    dest: &Path,
    want: impl Fn(&str) -> bool,
    dialect: &dyn XmlDialect,
    mut process_part: impl FnMut(&str, &str) -> Result<String>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let file = File::open(source)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    ensure_parent(dest)?;
    let temporary = TemporaryOutput(temporary_output(dest));
    let out = File::create(&temporary.0)?;
    let mut writer = ZipWriter::new(out);
    let opts = zip::write::FileOptions::default();
    for i in 0..zip.len() {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
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
            let raw = read_string_cancellable(&mut entry, is_cancelled)?;
            let rewritten = process_part(&name, &raw)?;
            if is_cancelled() {
                return Err(FilterError::Cancelled);
            }
            writer.write_all(rewritten.as_bytes())?;
        } else {
            copy_cancellable(&mut entry, &mut writer, is_cancelled)?;
        }
    }
    writer.finish().map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    #[cfg(windows)]
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(&temporary.0, dest)?;
    std::mem::forget(temporary);
    let _ = dialect;
    Ok(())
}

pub fn run_part(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
) -> Result<ProcessResult> {
    run_part_cfg(raw, dialect, hooks, EngineConfig::default())
}

pub fn run_part_cfg(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
) -> Result<ProcessResult> {
    run_part_cfg_cancellable(raw, dialect, hooks, cfg, &|| false)
}

pub fn run_part_cfg_cancellable(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ProcessResult> {
    process_xml_cancellable(raw, dialect, hooks, cfg, is_cancelled).map_err(|e| {
        if e == crate::xml_engine::CANCELLED_ERROR || is_cancelled() {
            FilterError::Cancelled
        } else {
            FilterError::Parse {
                format: "xml".into(),
                message: e,
            }
        }
    })
}
