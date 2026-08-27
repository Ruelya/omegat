//! Java `org.omegat.filters2.mozlang.MozillaLangFilter`.

use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct MozillaLangFilter;

impl Filter for MozillaLangFilter {
    fn id(&self) -> &'static str {
        "mozlang"
    }
    fn name(&self) -> &'static str {
        "Mozilla Lang"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.lang"]
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

enum State {
    WaitSource,
    WaitTarget,
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let source_re = Regex::new(r"^;(.*)").unwrap();
    let note_re = Regex::new(r"# (.*)").unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut source = String::new();
    let mut target = String::new();
    let mut state = State::WaitSource;

    for (line, _) in crate::text::lines_with_breaks(raw) {
        let s = line.trim();
        match state {
            State::WaitSource => {
                if let Some(cap) = source_re.captures(s) {
                    source.push_str(&cap[1]);
                    state = State::WaitTarget;
                }
                if note_re.is_match(s) {
                    // localization note kept only for comments; not required by golden
                }
                target.clear();
                written.push_str(s);
                written.push('\n');
            }
            State::WaitTarget => {
                target.push_str(s);
                let src = source.clone();
                // Java: translation is null when source == target (untranslated).
                let existing = if target.is_empty() || target == src {
                    None
                } else {
                    Some(target.clone())
                };
                segments.push(ExtractedSegment {
                    id: segments.len().to_string(),
                    source: src.clone(),
                    existing_translation: existing,
                    note: None,
                    comment: None,
                    path: None,
                    protected_parts: vec![],
                });
                let trans = match translations.and_then(|m| m.get(&src).cloned()) {
                    Some(t) if t == src => format!("{t} {{ok}}"),
                    Some(t) => t,
                    None => src,
                };
                written.push_str(&trans);
                written.push('\n');
                source.clear();
                target.clear();
                state = State::WaitSource;
            }
        }
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}
