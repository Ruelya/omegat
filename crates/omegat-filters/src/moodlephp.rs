//! Java `org.omegat.filters2.moodlephp.MoodlePHPFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct MoodlePhpFilter;

impl Filter for MoodlePhpFilter {
    fn id(&self) -> &'static str {
        "moodlephp"
    }
    fn name(&self) -> &'static str {
        "Moodle PHP"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.php"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None, ctx).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations), ctx).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

fn process(
    raw: &str,
    translations: Option<&HashMap<String, String>>,
    ctx: &FilterContext,
) -> Outcome {
    let remove_untranslated = ctx.option_flag("unremoveStringsUntranslated");
    let re = Regex::new(r"(?s)\$string\['(.+)'] (=) '(.+)(';)$").unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut block = String::new();
    let mut is_in_block = false;
    let quotes = '\'';
    let mut previous: char = '\0';

    for c in raw.chars() {
        if c == '$' && !is_in_block {
            is_in_block = true;
        }
        if is_in_block {
            block.push(c);
        } else {
            written.push(c);
        }
        if c == ';' && is_in_block && previous == quotes {
            is_in_block = false;
            process_block(
                &block,
                &re,
                translations,
                remove_untranslated,
                &mut segments,
                &mut written,
            );
            block.clear();
        }
        if c == quotes && previous == '\\' {
            previous = '\0';
        } else {
            previous = c;
        }
    }
    if !block.is_empty() {
        written.push_str(&block);
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn process_block(
    block: &str,
    re: &Regex,
    translations: Option<&HashMap<String, String>>,
    remove_untranslated: bool,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
) {
    let Some(m) = re.captures(block) else {
        written.push_str(block);
        return;
    };
    let id = m.get(1).unwrap().as_str();
    let text = m.get(3).unwrap().as_str();
    let text_range = m.get(3).unwrap();
    segments.push(seg(id, text));
    let (trans, found) = if let Some(map) = translations {
        if let Some(t) = map.get(id).cloned().or_else(|| map.get(text).cloned()) {
            (t, true)
        } else {
            (text.to_string(), false)
        }
    } else {
        (text.to_string(), true)
    };
    if found || !remove_untranslated {
        written.push_str(&block[..text_range.start()]);
        written.push_str(&trans);
        written.push_str(&block[text_range.end()..]);
    }
}
