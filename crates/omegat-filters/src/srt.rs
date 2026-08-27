//! Java `org.omegat.filters2.subtitles.SrtFilter`.

use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct SrtFilter;

impl Filter for SrtFilter {
    fn id(&self) -> &'static str {
        "srt"
    }
    fn name(&self) -> &'static str {
        "SubRip Subtitles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.srt"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(crate::subtitle::process_timed(&read_to_string(path)?, &time_re(), None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = crate::subtitle::process_timed(&read_to_string(source_path)?, &time_re(), Some(translations))
            .written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn time_re() -> Regex {
    Regex::new(r"^([0-9]{2}:[0-9]{2}:[0-9]{2},[0-9]{3})\s+-->\s+([0-9]{2}:[0-9]{2}:[0-9]{2},[0-9]{3})$")
        .unwrap()
}
