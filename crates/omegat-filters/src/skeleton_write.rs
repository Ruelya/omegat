use crate::{apply_skeleton_with_originals, ensure_parent, read_to_string, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
pub fn write_parsed(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    parsed: ParsedFile,
) -> Result<()> {
    let originals: Vec<String> = parsed.segments.iter().map(|s| s.source.clone()).collect();
    let out = parsed
        .skeleton
        .map(|sk| apply_skeleton_with_originals(&sk, translations, &originals))
        .unwrap_or_else(|| read_to_string(source_path).unwrap_or_default());
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, out)?;
    Ok(())
}
