//! Java `org.omegat.filters2.text.mozftl.MozillaFTLFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct MozillaFtlFilter;

impl Filter for MozillaFtlFilter {
    fn id(&self) -> &'static str {
        "mozftl"
    }
    fn name(&self) -> &'static str {
        "Mozilla FTL"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.ftl"]
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
    let attributes = Regex::new(r" +\.([^ ]+) =(.*)").unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut comments: Option<String> = None;
    let mut key: Option<String> = None;
    let mut k: Option<String> = None;
    let mut key_attr = String::new();
    let mut value: Option<String> = None;
    let mut multiline = false;
    let lines = crate::text::lines_with_breaks(raw);
    let mut i = 0usize;

    while i < lines.len() {
        let (str_line, br) = lines[i];
        let trimmed = str_line.trim();
        if trimmed.is_empty() {
            written.push_str(str_line);
            written.push_str(br);
            comments = None;
            i += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            written.push_str(str_line);
            written.push_str(br);
            comments = Some(match comments {
                None => str_line.to_string(),
                Some(c) => format!("{c}\n{str_line}"),
            });
            i += 1;
            continue;
        }

        let mut equals_pos = str_line.find('=');
        if equals_pos.is_none() || multiline {
            multiline = true;
            equals_pos = Some(str_line.len().saturating_sub(1));
        } else {
            key = Some(str_line[..equals_pos.unwrap()].trim().to_string());
        }
        let mut equals_pos = equals_pos.unwrap_or(0);
        while str_line[equals_pos..].chars().count() > 1 {
            let next = str_line[equals_pos..].chars().nth(1);
            if next != Some(' ') {
                break;
            }
            equals_pos += str_line[equals_pos..].chars().next().unwrap().len_utf8();
        }
        let after_eq = if equals_pos < str_line.len() {
            equals_pos + str_line[equals_pos..].chars().next().unwrap().len_utf8()
        } else {
            str_line.len()
        };

        let v = if multiline {
            str_line.to_string()
        } else {
            match &mut k {
                None => k = Some(str_line[..after_eq.min(str_line.len())].to_string()),
                Some(buf) => {
                    buf.push_str(br);
                    buf.push_str(&str_line[..after_eq.min(str_line.len())]);
                }
            }
            str_line.get(after_eq..).unwrap_or("").to_string()
        };
        value = Some(match value {
            None => v,
            Some(prev) => format!("{prev}\n{v}"),
        });
        if !multiline {
            let cur = key.clone().unwrap_or_default();
            key = Some(if cur == key_attr {
                cur
            } else {
                format!("{key_attr}{cur}")
            });
        }

        i += 1;
        let next = lines.get(i).map(|(l, b)| (*l, *b));
        if let Some((nxt, _)) = next {
            if !nxt.is_empty() {
                let cp = nxt.chars().next().unwrap();
                if cp == ' ' && !attributes.is_match(nxt) {
                    multiline = true;
                    continue;
                }
                if cp == ' ' {
                    if key_attr.is_empty() {
                        key_attr = key.clone().unwrap_or_default();
                    }
                    if value.as_deref().unwrap_or("").is_empty() {
                        value = None;
                        continue;
                    }
                } else {
                    key_attr.clear();
                }
            } else {
                key_attr.clear();
            }
        } else {
            key_attr.clear();
        }

        let key_s = key.clone().unwrap_or_default();
        let val_s = value.clone().unwrap_or_default();
        let mut seg = seg(&key_s, &val_s);
        seg.comment = comments.clone();
        segments.push(seg);

        let (trans, found) = if let Some(map) = translations {
            if let Some(t) = map.get(&key_s).cloned().or_else(|| map.get(&val_s).cloned()) {
                (t.replace('\n', br), true)
            } else {
                (val_s, false)
            }
        } else {
            (val_s, true)
        };
        if found || !remove_untranslated {
            if let Some(prefix) = &k {
                written.push_str(prefix);
            }
            written.push_str(&trans);
            let emit_br = next.map(|(_, b)| b).unwrap_or(br);
            written.push_str(emit_br);
        }
        k = None;
        multiline = false;
        value = None;
        comments = None;
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}
