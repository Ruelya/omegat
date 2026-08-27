//! Java `org.omegat.filters2.text.magento.MagentoFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct MagentoFilter;

impl Filter for MagentoFilter {
    fn id(&self) -> &'static str {
        "magento"
    }
    fn name(&self) -> &'static str {
        "Magento CE Locale CSV"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.csv"]
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
    let mut segments = Vec::new();
    let mut written = String::new();
    let lines = crate::text::lines_with_breaks(raw);
    let mut i = 0usize;
    while i < lines.len() {
        let (mut line, mut br) = (lines[i].0.to_string(), lines[i].1);
        i += 1;
        while !line.ends_with('"') && i < lines.len() {
            line.push_str(br);
            line.push_str(lines[i].0);
            br = lines[i].1;
            i += 1;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            written.push_str(&line);
            written.push_str(br);
            continue;
        }
        let result = split_outside_quotes(trimmed);
        if result.len() < 2 {
            written.push_str(&line);
            written.push_str(br);
            continue;
        }
        let key = strip_outer_quotes(&result[0]);
        let value = strip_outer_quotes(&result[1]);
        segments.push(seg(&key, &value));
        let trans = if let Some(map) = translations {
            map.get(&key)
                .cloned()
                .or_else(|| map.get(&value).cloned())
                .unwrap_or(value)
        } else {
            value
        };
        written.push('"');
        written.push_str(&key);
        written.push_str("\",\"");
        written.push_str(&trans);
        written.push('"');
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

/// Java `, (?= (?: [^"]* "[^"]*" )* (?! [^"]* ") )` — commas outside quotes.
fn split_outside_quotes(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in s.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            cur.push(c);
        } else if c == ',' && !in_quotes {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

fn strip_outer_quotes(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
