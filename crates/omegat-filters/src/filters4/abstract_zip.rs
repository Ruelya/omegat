//! Java `AbstractZipFilter`.

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

pub fn parse_zip_parts(
    path: &Path,
    mut accept: impl FnMut(&str) -> bool,
    mut translate: impl FnMut(&str) -> bool,
    mut parse_part: impl FnMut(&str, &str) -> Result<Vec<ExtractedSegment>>,
    mut cmp: Option<impl FnMut(&str, &str) -> std::cmp::Ordering>,
) -> Result<Vec<ExtractedSegment>> {
    parse_zip_parts_cancellable(
        path,
        accept,
        translate,
        parse_part,
        cmp,
        &|| false,
    )
}

pub fn parse_zip_parts_cancellable(
    path: &Path,
    mut accept: impl FnMut(&str) -> bool,
    mut translate: impl FnMut(&str) -> bool,
    mut parse_part: impl FnMut(&str, &str) -> Result<Vec<ExtractedSegment>>,
    mut cmp: Option<impl FnMut(&str, &str) -> std::cmp::Ordering>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ExtractedSegment>> {
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    let _ = accept("");
    let mut names: Vec<String> = (0..zip.len())
        .filter_map(|i| {
            if is_cancelled() {
                return None;
            }
            zip.by_index(i).ok().map(|e| e.name().to_string())
        })
        .filter(|n| translate(n))
        .collect();
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    if let Some(cmp) = &mut cmp {
        names.sort_by(|a, b| cmp(a, b));
    }
    let mut segments = Vec::new();
    for name in names {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
        let mut entry = zip.by_name(&name).map_err(|e| FilterError::Parse {
            format: "zip".into(),
            message: e.to_string(),
        })?;
        let raw = crate::xml_zip::read_string_cancellable(&mut entry, is_cancelled)?;
        segments.extend(parse_part(&name, &raw)?);
    }
    if is_cancelled() {
        Err(FilterError::Cancelled)
    } else {
        Ok(segments)
    }
}

pub fn write_zip_parts(
    source: &Path,
    dest: &Path,
    mut translate: impl FnMut(&str) -> bool,
    mut delete: impl FnMut(&str) -> bool,
    mut rewrite: impl FnMut(&str, &str) -> Result<String>,
) -> Result<()> {
    write_zip_parts_cancellable(
        source,
        dest,
        translate,
        delete,
        rewrite,
        &|| false,
    )
}

pub fn write_zip_parts_cancellable(
    source: &Path,
    dest: &Path,
    mut translate: impl FnMut(&str) -> bool,
    mut delete: impl FnMut(&str) -> bool,
    mut rewrite: impl FnMut(&str, &str) -> Result<String>,
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
        if delete(&name) {
            continue;
        }
        writer
            .start_file(name.clone(), opts)
            .map_err(|e| FilterError::Parse {
                format: "zip".into(),
                message: e.to_string(),
            })?;
        if translate(&name) {
            let raw = crate::xml_zip::read_string_cancellable(&mut entry, is_cancelled)?;
            let rewritten = rewrite(&name, &raw)?;
            if is_cancelled() {
                return Err(FilterError::Cancelled);
            }
            writer.write_all(rewritten.as_bytes())?;
        } else {
            let mut chunk = [0u8; 64 * 1024];
            loop {
                if is_cancelled() {
                    return Err(FilterError::Cancelled);
                }
                let count = entry.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                writer.write_all(&chunk[..count])?;
            }
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
    Ok(())
}

pub fn short_name(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}
