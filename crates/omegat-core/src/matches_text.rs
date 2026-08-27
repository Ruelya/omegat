//! Java `org.omegat.gui.matches.MatchesTextArea.substituteNumbers`.

use crate::string_util::normalize_width;
use crate::tokenize::tokenize_verbatim;
use std::collections::HashSet;

/// Substitute numbers from `source` into `target_match` when the fuzzy match
/// and the current source have the same number multiset (width-normalized).
pub fn substitute_numbers(source: &str, source_match: &str, target_match: &str) -> String {
    let src_toks = merge_ascii_decimals(tokenize_verbatim(source));
    let sm_toks = merge_ascii_decimals(tokenize_verbatim(source_match));
    let tm_toks = merge_ascii_decimals(tokenize_verbatim(target_match));

    let src_nums: Vec<String> = src_toks.iter().filter(|t| is_number(t)).cloned().collect();
    let sm_nums: Vec<String> = sm_toks.iter().filter(|t| is_number(t)).cloned().collect();
    let tm_nums: Vec<String> = tm_toks.iter().filter(|t| is_number(t)).cloned().collect();

    if sm_nums.len() != src_nums.len() || sm_nums.len() != tm_nums.len() {
        return target_match.to_string();
    }
    let norm_sm: Vec<String> = sm_nums.iter().map(|s| normalize_width(s)).collect();
    let norm_tm: Vec<String> = tm_nums.iter().map(|s| normalize_width(s)).collect();
    let set_sm: HashSet<&String> = norm_sm.iter().collect();
    let set_tm: HashSet<&String> = norm_tm.iter().collect();
    if set_sm != set_tm {
        return target_match.to_string();
    }

    let map = map_target_to_source(&norm_sm, &norm_tm);
    let mut i = 0usize;
    let mut result = String::new();
    for tok in &tm_toks {
        if is_number(tok) {
            result.push_str(&to_digit_width_of(&src_nums[map[i]], tok));
            i += 1;
        } else {
            result.push_str(tok);
        }
    }
    result
}

fn merge_ascii_decimals(toks: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if is_ascii_int(&toks[i])
            && i + 2 < toks.len()
            && toks[i + 1] == "."
            && is_ascii_int(&toks[i + 2])
        {
            out.push(format!("{}.{}", toks[i], toks[i + 2]));
            i += 3;
        } else {
            out.push(toks[i].clone());
            i += 1;
        }
    }
    out
}

fn is_ascii_int(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Java `Integer.parseInt` (Unicode digits) or ASCII `Double.parseDouble`.
pub fn is_number(text: &str) -> bool {
    java_parse_int(text).is_some() || text.parse::<f64>().is_ok()
}

fn java_parse_int(text: &str) -> Option<i32> {
    if text.is_empty() {
        return None;
    }
    let mut s = text;
    let mut sign = 1i32;
    if let Some(rest) = s.strip_prefix('-') {
        sign = -1;
        s = rest;
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    }
    if s.is_empty() {
        return None;
    }
    let mut n: i32 = 0;
    for c in s.chars() {
        let d = unicode_decimal_digit(c)?;
        n = n.checked_mul(10)?.checked_add(d as i32)?;
    }
    Some(n * sign)
}

/// Java `Character.digit(c, 10)` / `Integer.parseInt` (ASCII, fullwidth, Arabic-Indic).
fn unicode_decimal_digit(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    let u = c as u32;
    if (0xFF10..=0xFF19).contains(&u) {
        return Some(u - 0xFF10);
    }
    if (0x0660..=0x0669).contains(&u) {
        return Some(u - 0x0660);
    }
    if (0x06F0..=0x06F9).contains(&u) {
        return Some(u - 0x06F0);
    }
    None
}

fn map_target_to_source(src_match: &[String], tgt_match: &[String]) -> Vec<usize> {
    let mut used = vec![false; src_match.len()];
    let mut out = vec![0; tgt_match.len()];
    for (j, t) in tgt_match.iter().enumerate() {
        for (i, s) in src_match.iter().enumerate() {
            if s == t && !used[i] {
                used[i] = true;
                out[j] = i;
                break;
            }
        }
    }
    out
}

fn has_fullwidth_digit(text: &str) -> bool {
    text.chars().any(|c| ('\u{FF10}'..='\u{FF19}').contains(&c))
}

fn to_fullwidth_digits(number: &str) -> String {
    number
        .chars()
        .map(|c| {
            if c.is_ascii_digit() {
                char::from_u32(0xFF10 + u32::from(c as u8 - b'0')).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn to_digit_width_of(number: &str, template: &str) -> String {
    if has_fullwidth_digit(template) {
        to_fullwidth_digits(number)
    } else {
        normalize_width(number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullwidth_source_digit_becomes_halfwidth_in_latin_target() {
        assert_eq!(
            substitute_numbers(
                "これは例文９です",
                "これは例文8です",
                "This is a sample sentence 8"
            ),
            "This is a sample sentence 9"
        );
    }
}
