//! Java `org.omegat.filters2.text.bundles.ResourceBundleFilter`.

use crate::misc::seg;
use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct PropertiesFilter;

impl Filter for PropertiesFilter {
    fn id(&self) -> &'static str {
        "properties"
    }
    fn name(&self) -> &'static str {
        "Java Resource Bundles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.properties"]
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
    let dont_unescape = ctx.option_flag("dontUnescapeULiterals");
    let remove_untranslated = ctx.option_flag("unremoveStringsUntranslated");
    let force_escape = ctx.option_flag("forceJava8LiteralsEscape");
    let dont_translate_comment = !matches!(
        ctx.option("dontTargetCommentValue").map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("false")
    );

    let mut segments: Vec<ExtractedSegment> = Vec::new();
    let mut written = String::new();
    let mut comments: Option<String> = None;
    let mut noi18n = false;
    let lines = crate::text::lines_with_breaks(raw);
    let mut idx = 0usize;

    while idx < lines.len() {
        let (raw_line, br) = lines[idx];
        idx += 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            written.push_str(raw_line);
            written.push_str(br);
            comments = None;
            continue;
        }

        let processed = match normalize_input_line(raw_line, dont_unescape) {
            Ok(s) => s,
            Err(_) => raw_line.to_string(),
        };

        let first = trimmed.chars().next().unwrap();
        if first == '#' || first == '!' {
            written.push_str(&to_ascii(raw_line, EscapeMode::Comment, force_escape));
            written.push_str(br);
            comments = Some(match comments {
                None => processed,
                Some(c) => format!("{c}\n{processed}"),
            });
            if raw_line.contains("NOI18N") {
                noi18n = true;
            }
            continue;
        }

        let mut processed = processed;
        while processed
            .chars()
            .next_back()
            .is_some_and(|c| c == '\\')
        {
            let next = if idx < lines.len() {
                let (n, _) = lines[idx];
                idx += 1;
                n
            } else {
                ""
            };
            processed.pop();
            if let Ok(n) = normalize_input_line(next, dont_unescape) {
                processed.push_str(&n);
            }
        }

        let equals_pos = search_equals(&processed);
        let key = if equals_pos >= 0 {
            remove_extra_slashes(processed[..equals_pos as usize].trim(), dont_unescape)
        } else {
            remove_extra_slashes(processed.trim(), dont_unescape)
        };

        if equals_pos >= 0 {
            let mut equals_end = equals_pos as usize + 1;
            let chars: Vec<char> = processed.chars().collect();
            let eq_idx = processed[..equals_pos as usize].chars().count();
            let mut ci = eq_idx + 1;
            while ci < chars.len() && (chars[ci] == ' ' || chars[ci] == '\t') {
                equals_end += chars[ci].len_utf8();
                ci += 1;
            }
            let equals = processed[equals_pos as usize..equals_end].to_string();
            let value = if equals_end < processed.len() {
                remove_extra_slashes(&processed[equals_end..], dont_unescape)
            } else {
                String::new()
            };

            if noi18n && dont_translate_comment {
                written.push_str(&to_ascii(&key, EscapeMode::Key, force_escape));
                written.push_str(&equals);
                written.push_str(&to_ascii(&value, EscapeMode::Value, force_escape));
                written.push_str(br);
                noi18n = false;
            } else {
                let value = value.replace("\n\n", "\n \n");
                segments.push({
                    let mut s = seg(&key, &value);
                    s.comment = comments.clone();
                    s
                });
                comments = None;
                let (trans, translated) = if let Some(map) = translations {
                    if let Some(t) = map.get(&key).cloned().or_else(|| map.get(&value).cloned()) {
                        (t, true)
                    } else {
                        (value.clone(), false)
                    }
                } else {
                    (value.clone(), true)
                };
                let mut trans = trans.replace("\n \n", "\n\n");
                trans = to_ascii(&trans, EscapeMode::Value, force_escape);
                if trans.starts_with(' ') {
                    trans = format!("\\{trans}");
                }
                if translated || !remove_untranslated {
                    written.push_str(&to_ascii(&key, EscapeMode::Key, force_escape));
                    written.push_str(&equals);
                    written.push_str(&trans);
                    written.push_str(br);
                }
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

#[derive(Clone, Copy)]
enum EscapeMode {
    Key,
    Value,
    Comment,
}

fn normalize_input_line(line: &str, dont_unescape: bool) -> std::result::Result<String, ()> {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    let mut stripping = true;
    while i < chars.len() {
        let mut cp = chars[i];
        if stripping {
            if cp.is_whitespace() {
                i += 1;
                continue;
            }
            stripping = false;
        }
        if cp == '\\' && i + 1 < chars.len() {
            i += 1;
            cp = chars[i];
            if cp != 'u' {
                if cp == 'n' {
                    cp = '\n';
                } else if cp == 'r' {
                    cp = '\r';
                } else if cp == 't' {
                    cp = '\t';
                } else {
                    result.push('\\');
                }
            } else if dont_unescape {
                result.push('\\');
            } else {
                if i + 4 >= chars.len() {
                    return Err(());
                }
                let hex: String = chars[i + 1..i + 5].iter().collect();
                let parsed = u32::from_str_radix(&hex, 16).map_err(|_| ())?;
                cp = char::from_u32(parsed).ok_or(())?;
                i += 4;
            }
        }
        result.push(cp);
        i += 1;
    }
    Ok(result)
}

fn to_ascii(text: &str, mode: EscapeMode, force_escape: bool) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let cp = chars[i];
        if !matches!(mode, EscapeMode::Comment) && cp == '\\' {
            if matches!(mode, EscapeMode::Value | EscapeMode::Key) {
                result.push_str("\\\\");
            }
        } else if cp == '\n' {
            result.push_str("\\n");
        } else if cp == '\r' {
            result.push_str("\\r");
        } else if cp == '\t' {
            result.push_str("\\t");
        } else if matches!(mode, EscapeMode::Key) && cp == ' ' {
            result.push_str("\\ ");
        } else if matches!(mode, EscapeMode::Key) && cp == '=' {
            result.push_str("\\=");
        } else if matches!(mode, EscapeMode::Key) && cp == ':' {
            result.push_str("\\:");
        } else if (cp as u32) >= 32 && (cp as u32) < 127 {
            result.push(cp);
        } else if !force_escape {
            result.push(cp);
        } else {
            let mut buf = [0u16; 2];
            let enc = cp.encode_utf16(&mut buf);
            for u in enc {
                result.push_str(&format!("\\u{u:04X}"));
            }
        }
        i += 1;
    }
    result
}

fn remove_extra_slashes(string: &str, dont_unescape: bool) -> String {
    let mut result = String::new();
    let chars: Vec<char> = string.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let mut cp = chars[i];
        if cp == '\\' && !(dont_unescape && contains_u_escape_at(string, i)) {
            if i + 1 < chars.len() {
                i += 1;
                cp = chars[i];
            } else {
                cp = ' ';
            }
        }
        result.push(cp);
        i += 1;
    }
    result
}

fn contains_u_escape_at(text: &str, offset_chars: usize) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if offset_chars + 6 > chars.len() {
        return false;
    }
    if chars[offset_chars + 1] != 'u' {
        return false;
    }
    let hex: String = chars[offset_chars + 2..offset_chars + 6].iter().collect();
    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).is_some()
}

fn search_equals(s: &str) -> i32 {
    let chars: Vec<char> = s.chars().collect();
    let mut prev = 'a';
    let mut byte = 0i32;
    for (i, &cp) in chars.iter().enumerate() {
        if prev != '\\' {
            if cp == '=' || cp == ':' {
                return byte;
            } else if cp == ' ' || cp == '\t' {
                let mut j = i + 1;
                while j < chars.len() {
                    let cp2 = chars[j];
                    if cp2 == ':' || cp2 == '=' {
                        return chars[..j].iter().map(|c| c.len_utf8() as i32).sum();
                    }
                    if cp2 != ' ' && cp2 != '\t' {
                        return byte;
                    }
                    j += 1;
                }
                return byte;
            }
        }
        prev = cp;
        byte += cp.len_utf8() as i32;
    }
    -1
}

