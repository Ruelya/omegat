//! Java `org.omegat.filters2.mozdtd.MozillaDTDFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct MozillaDtdFilter;

impl Filter for MozillaDtdFilter {
    fn id(&self) -> &'static str {
        "mozdtd"
    }
    fn name(&self) -> &'static str {
        "Mozilla DTD"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.dtd"]
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

fn process(raw: &str, translations: Option<&HashMap<String, String>>, ctx: &FilterContext) -> Outcome {
    let remove_untranslated = ctx.option_flag("unremoveStringsUntranslated");
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut block = String::new();
    let mut is_in_block = false;
    let mut found_quotes = false;
    let mut quotes = '"';
    let mut previous: char = '\0';

    for c in raw.chars() {
        if c == '<' && !is_in_block {
            is_in_block = true;
        }
        if is_in_block {
            block.push(c);
        } else {
            written.push(c);
        }
        if !found_quotes && (c == '"' || c == '\'') {
            found_quotes = true;
            quotes = c;
        }
        if c == '>' && is_in_block && previous == quotes {
            is_in_block = false;
            found_quotes = false;
            process_block(
                &block,
                translations,
                remove_untranslated,
                &mut segments,
                &mut written,
            );
            block.clear();
        } else if c == '>' && is_in_block && previous == '-' {
            is_in_block = false;
            found_quotes = false;
            written.push_str(&block);
            block.clear();
        }
        if !c.is_whitespace() {
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
    translations: Option<&HashMap<String, String>>,
    remove_untranslated: bool,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
) {
    let Some((id, text, text_start, text_end)) = parse_entity(block) else {
        written.push_str(block);
        return;
    };
    segments.push(seg(&id, &text));
    let (trans, found) = if let Some(map) = translations {
        if let Some(t) = map.get(&id).cloned().or_else(|| map.get(&text).cloned()) {
            (t, true)
        } else {
            (text.clone(), false)
        }
    } else {
        (text, true)
    };
    if found || !remove_untranslated {
        written.push_str(&block[..text_start]);
        written.push_str(&trans);
        written.push_str(&block[text_end..]);
    }
}

/// Java `RE_ENTITY`: `<!ENTITY\s+(\S+)\s+(["'])(.+)\2\s*>` (DOTALL).
fn parse_entity(block: &str) -> Option<(String, String, usize, usize)> {
    if !block.starts_with("<!ENTITY") {
        return None;
    }
    let bytes = block.as_bytes();
    let mut i = "<!ENTITY".len();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let id_start = i;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i == id_start {
        return None;
    }
    let id = block[id_start..i].to_string();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let quote = bytes[i] as char;
    if quote != '"' && quote != '\'' {
        return None;
    }
    i += 1;
    let text_start = i;
    let rest = &block[text_start..];
    let close = rest.rfind(quote)?;
    if close == 0 {
        return None;
    }
    if rest[close + 1..].trim() != ">" {
        return None;
    }
    let text = rest[..close].to_string();
    Some((id, text, text_start, text_start + close))
}
