//! Java `org.omegat.filters2.rc.RcFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct RcFilter;

impl Filter for RcFilter {
    fn id(&self) -> &'static str {
        "rc"
    }
    fn name(&self) -> &'static str {
        "Windows Resources"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.rc"]
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Dialog,
    Menu,
    MessageTable,
    StringTable,
    Other,
    Unknown,
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let re_dialog = Regex::new(r"(?s)(\S+)\s+DIALOG(EX)?\s+.+").unwrap();
    let re_dialog_caption = Regex::new(r"CAPTION\s+.+").unwrap();
    let re_menu = Regex::new(r"(?s)(\S+)\s+MENU(EX)?\s*.*").unwrap();
    let re_msg = Regex::new(r"(?s)(\S+)\s+MESSAGETABLE\s*.*").unwrap();
    let re_str = Regex::new(r"STRINGTABLE\s*.*").unwrap();

    let mut segments = Vec::new();
    let mut written = String::new();
    let mut c_part = Part::Unknown;
    let mut c_level = 0i32;
    let mut block_id = String::new();

    for (s_line, _) in crate::text::lines_with_breaks(raw) {
        let mut s = s_line.to_string();
        let mut b: i32 = -1;
        let mut e: i32 = -1;
        let mut id: Option<String> = None;
        let strim = s.trim();

        if strim.starts_with("//") || strim.starts_with('#') {
            written.push_str(&s);
            written.push('\n');
            continue;
        }

        if strim.is_empty() {
            if c_level == 0 {
                c_part = Part::Unknown;
            }
        } else if c_part == Part::Unknown {
            if let Some(m) = re_dialog.captures(strim) {
                block_id = m[1].to_string();
                c_part = Part::Dialog;
            } else if let Some(m) = re_menu.captures(strim) {
                block_id = m[1].to_string();
                c_part = Part::Menu;
            } else if let Some(m) = re_msg.captures(strim) {
                block_id = m[1].to_string();
                c_part = Part::MessageTable;
            } else if re_str.is_match(strim) {
                block_id.clear();
                c_part = Part::StringTable;
            } else {
                c_part = Part::Other;
            }
        } else if strim == "{" || strim.eq_ignore_ascii_case("BEGIN") {
            c_level += 1;
        } else if strim == "}" || strim.eq_ignore_ascii_case("END") {
            c_level -= 1;
            if c_level == 0 {
                c_part = Part::Unknown;
            }
        } else if c_level > 0 && c_part != Part::Other {
            if let Some((nb, ne)) = mark_for_translation(&s) {
                b = nb;
                e = ne;
                if b >= 0 && e >= 0 && b < e {
                    id = parse_id(c_part, &s, b as usize, e as usize);
                }
            }
        } else if c_level == 0 && c_part == Part::Dialog && re_dialog_caption.is_match(strim) {
            if let Some((nb, ne)) = mark_for_translation(&s) {
                b = nb;
                e = ne;
                id = Some("__CAPTION__".into());
            }
        }

        if b >= 0 && e >= 0 && b < e {
            let mut loc = s[b as usize + 1..e as usize].to_string();
            loc = loc.replace("\\\"", "\"").replace("\"\"", "\"");
            if !loc.is_empty() {
                let full_id = format!("{}/{}", block_id, id.as_deref().unwrap_or(""));
                segments.push(seg(&full_id, &loc));
                if let Some(map) = translations {
                    let trans = map
                        .get(&full_id)
                        .cloned()
                        .or_else(|| map.get(&loc).cloned())
                        .unwrap_or(loc);
                    let trans = trans.replace('"', "\"\"");
                    s = format!("{}{}{}", &s[..b as usize + 1], trans, &s[e as usize..]);
                }
            }
        }
        written.push_str(&s);
        written.push('\n');
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn parse_id(c_part: Part, line: &str, _b: usize, e: usize) -> Option<String> {
    match c_part {
        Part::Dialog | Part::Menu => {
            let w: Vec<&str> = line[e..].split(',').collect();
            if w.len() > 1 {
                Some(w[1].trim().to_string())
            } else {
                None
            }
        }
        Part::MessageTable | Part::StringTable => {
            let w: Vec<&str> = line[.._b].split(',').collect();
            Some(w[0].trim().to_string())
        }
        _ => None,
    }
}

fn mark_for_translation(s: &str) -> Option<(i32, i32)> {
    let b = s.find('"')? as i32;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut e = b;
    loop {
        let e_usize = e as usize;
        let Some(pos) = s[e_usize + 1..].find('"') else {
            return None;
        };
        e = (e_usize + 1 + pos) as i32;
        let before = s[..e as usize].chars().next_back();
        if before == Some('\\') {
            continue;
        }
        let after = s[e as usize + 1..].chars().next();
        if after == Some('"') {
            e += 1;
            continue;
        }
        break;
    }
    let _ = chars;
    Some((b, e))
}
