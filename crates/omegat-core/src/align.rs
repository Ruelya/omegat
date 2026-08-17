use crate::error::Result;
use crate::tmx::{ProjectTmx, TmxEntry};
use std::path::Path;

/// Pair lines / paragraphs from two files into a TMX.
pub fn align_files(source: &Path, target: &Path, src_lang: &str, tgt_lang: &str) -> Result<ProjectTmx> {
    let left = std::fs::read_to_string(source)?;
    let right = std::fs::read_to_string(target)?;
    let ls: Vec<&str> = left.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let rs: Vec<&str> = right.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let mut tmx = ProjectTmx::new();
    for (a, b) in ls.iter().zip(rs.iter()) {
        tmx.insert(TmxEntry {
            source: (*a).to_string(),
            translation: (*b).to_string(),
            default_translation: true,
            ..Default::default()
        });
    }
    let _ = (src_lang, tgt_lang);
    Ok(tmx)
}

pub fn write_aligned_tmx(tmx: &ProjectTmx, dest: &Path, src_lang: &str, tgt_lang: &str) -> Result<()> {
    tmx.write(dest, src_lang, tgt_lang)
}
