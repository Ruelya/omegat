//! Java `AbstractZipFilter`.

use crate::{ensure_parent, ExtractedSegment, FilterError, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::{ZipArchive, ZipWriter};

pub fn parse_zip_parts(
    path: &Path,
    mut accept: impl FnMut(&str) -> bool,
    mut translate: impl FnMut(&str) -> bool,
    mut parse_part: impl FnMut(&str, &str) -> Result<Vec<ExtractedSegment>>,
    mut cmp: Option<impl FnMut(&str, &str) -> std::cmp::Ordering>,
) -> Result<Vec<ExtractedSegment>> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| FilterError::Parse {
        format: "zip".into(),
        message: e.to_string(),
    })?;
    let _ = accept("");
    let mut names: Vec<String> = (0..zip.len())
        .filter_map(|i| {
            zip.by_index(i).ok().map(|e| e.name().to_string())
        })
        .filter(|n| translate(n))
        .collect();
    if let Some(cmp) = &mut cmp {
        names.sort_by(|a, b| cmp(a, b));
    }
    let mut segments = Vec::new();
    for name in names {
        let mut entry = zip.by_name(&name).map_err(|e| FilterError::Parse {
            format: "zip".into(),
            message: e.to_string(),
        })?;
        let mut raw = String::new();
        if entry.read_to_string(&mut raw).is_err() {
            continue;
        }
        segments.extend(parse_part(&name, &raw)?);
    }
    Ok(segments)
}

pub fn write_zip_parts(
    source: &Path,
    dest: &Path,
    mut translate: impl FnMut(&str) -> bool,
    mut delete: impl FnMut(&str) -> bool,
    mut rewrite: impl FnMut(&str, &str) -> Result<String>,
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
            let mut raw = String::new();
            entry.read_to_string(&mut raw)?;
            let rewritten = rewrite(&name, &raw)?;
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
    Ok(())
}

pub fn short_name(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}
