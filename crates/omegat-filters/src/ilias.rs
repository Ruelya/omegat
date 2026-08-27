//! Java `org.omegat.filters2.text.ilias.ILIASFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct IliasFilter;

impl Filter for IliasFilter {
    fn id(&self) -> &'static str {
        "ilias"
    }
    fn name(&self) -> &'static str {
        "ILIAS Language File"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.lang", "*.lang.local"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let pattern = Regex::new(r"^(\S+)#:#(\S+)#:#(.+)$").unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    for (line, br) in crate::text::lines_with_breaks(raw) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            written.push_str(line);
            written.push_str(br);
            continue;
        }
        let Some(mat) = pattern.captures(line) else {
            written.push_str(line);
            written.push_str(br);
            continue;
        };
        let key = format!("{}#:#{}", &mat[1], &mat[2]);
        let value = mat[3].to_string();
        if value.is_empty() {
            written.push_str(line);
            written.push_str(br);
            continue;
        }
        segments.push(seg(&key, &value));
        let trans = if let Some(map) = translations {
            map.get(&key)
                .cloned()
                .or_else(|| map.get(&value).cloned())
                .unwrap_or(value)
        } else {
            value
        };
        written.push_str(&key);
        written.push_str("#:#");
        written.push_str(&trans);
        written.push_str(br);
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}
