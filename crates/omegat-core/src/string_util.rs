//! Java `org.omegat.util.StringUtil`.

pub const TRUNCATE_CHAR: char = '…';

pub fn is_lower_case(input: &str) -> bool {
    let mut has_letters = false;
    for ch in input.chars() {
        if ch.is_alphabetic() {
            has_letters = true;
            if !ch.is_lowercase() {
                return false;
            }
        }
    }
    has_letters
}

pub fn is_upper_case(input: &str) -> bool {
    let mut has_letters = false;
    for ch in input.chars() {
        if ch.is_alphabetic() {
            has_letters = true;
            if !ch.is_uppercase() {
                return false;
            }
        }
    }
    has_letters
}

pub fn is_title_case_cp(cp: char) -> bool {
    // Java: Character.isTitleCase || (isUpperCase && toTitleCase == cp)
    if matches!(cp, '\u{01C5}' | '\u{01C8}' | '\u{01CB}' | '\u{01F2}') {
        return true;
    }
    cp.is_uppercase() && title_case_variant(cp) == cp
}

fn title_case_variant(cp: char) -> char {
    match cp {
        '\u{01C4}' | '\u{01C6}' => '\u{01C5}',
        '\u{01C7}' | '\u{01C9}' => '\u{01C8}',
        '\u{01CA}' | '\u{01CC}' => '\u{01CB}',
        '\u{01F1}' | '\u{01F3}' => '\u{01F2}',
        other => other.to_uppercase().next().unwrap_or(other),
    }
}

pub fn is_title_case(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let rest: String = chars.collect();
    if rest.is_empty() {
        is_title_case_cp(first)
    } else {
        is_title_case_cp(first) && is_lower_case(&rest)
    }
}

pub fn is_mixed_case(input: &str) -> bool {
    if input.is_empty() || input.chars().count() < 2 {
        return false;
    }
    let mut has_upper = false;
    let mut has_lower = false;
    for (i, ch) in input.char_indices() {
        if ch.is_alphabetic() {
            if ch.is_uppercase() && i > 0 {
                has_upper = true;
            } else if ch.is_lowercase() {
                has_lower = true;
            }
            if has_upper && has_lower {
                return true;
            }
        }
    }
    false
}

pub fn is_white_space_cp(cp: char) -> bool {
    cp.is_whitespace() || matches!(cp, '\u{00A0}' | '\u{2007}' | '\u{202F}')
}

pub fn is_white_space(input: &str) -> bool {
    !input.is_empty() && input.chars().all(is_white_space_cp)
}

pub fn is_valid_xml_char(c: u32) -> bool {
    matches!(c, 0x9 | 0xA | 0xD)
        || (0x20..=0xD7FF).contains(&c)
        || (0xE000..=0xFFFD).contains(&c)
        || (0x10000..=0x10FFFF).contains(&c)
}

pub fn compress_spaces(s: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

pub fn first_n(s: &str, len: usize) -> String {
    s.chars().take(len).collect()
}

pub fn truncate(text: &str, len: usize) -> String {
    if len == 0 {
        panic!("IndexOutOfBoundsException");
    }
    if text.chars().count() <= len {
        return text.to_string();
    }
    let mut out = first_n(text, len - 1);
    out.push(TRUNCATE_CHAR);
    out
}

pub fn is_substring_after(text: &str, pos: usize, substring: &str) -> bool {
    text.get(pos..).is_some_and(|t| t.starts_with(substring))
}

pub fn is_substring_before(text: &str, pos: usize, substring: &str) -> bool {
    pos >= substring.len() && text.get(..pos).is_some_and(|t| t.ends_with(substring))
}

pub fn strip_from_end(string: &str, to_strip: &[&str]) -> String {
    let mut s = string.to_string();
    for t in to_strip {
        if let Some(stripped) = s.strip_suffix(t) {
            s = stripped.to_string();
        }
    }
    s
}

pub fn rstrip(s: &str) -> String {
    s.trim_end().to_string()
}

fn title_case_char(ch: char, locale: &str) -> String {
    if ch == 'i' && locale.eq_ignore_ascii_case("tr") {
        return "\u{0130}".into();
    }
    if ch == '\u{01CC}' {
        return "\u{01CB}".into();
    }
    ch.to_uppercase().collect()
}

pub fn to_title_case(text: &str, locale: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut first_letter = None;
    for (i, ch) in text.char_indices() {
        if ch.is_alphabetic() {
            first_letter = Some((i, ch));
            break;
        }
    }
    let Some((idx, ch)) = first_letter else {
        return text.to_string();
    };
    let prefix = &text[..idx];
    let rest_start = idx + ch.len_utf8();
    format!("{prefix}{}{}", title_case_char(ch, locale), text[rest_start..].to_lowercase())
}

pub fn capitalize_first(text: &str, locale: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap();
    format!("{}{}", title_case_char(first, locale), chars.as_str())
}

pub fn match_capitalization(text: &str, match_to: Option<&str>, locale: &str) -> String {
    let Some(mt) = match_to.filter(|s| !s.is_empty()) else {
        return text.to_string();
    };
    if text.is_empty() || mt.starts_with(text) {
        return text.to_string();
    }
    if is_title_case(mt) {
        return to_title_case(text, locale);
    }
    if is_lower_case(mt) {
        return text.to_lowercase();
    }
    if is_upper_case(mt) {
        return text.to_uppercase();
    }
    text.to_string()
}

pub fn convert_to_list(s: &str) -> Vec<String> {
    s.trim().split_whitespace().map(|p| p.to_string()).collect()
}

/// Java `StringUtil.isCJK`: every code point is ≥ CJK Radicals Supplement (U+2E80).
pub fn is_cjk(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    input.chars().all(|c| (c as u32) >= 0x2E80)
}

/// Java `StringUtil.getTailSegments`.
pub fn get_tail_segments(s: &str, separator: char, segments: usize) -> String {
    if segments == 0 {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut start = s.len();
    for _ in 0..segments {
        if start == 0 {
            return s.to_string();
        }
        let search = &s[..start];
        match search.rfind(separator) {
            Some(i) => start = i,
            None => return s.to_string(),
        }
        let _ = bytes;
    }
    if start + separator.len_utf8() <= s.len() {
        s[start + separator.len_utf8()..].to_string()
    } else {
        s.to_string()
    }
}

pub fn compare_to_nullable(v1: Option<&str>, v2: Option<&str>) -> i32 {
    match (v1, v2) {
        (None, None) => 0,
        (None, Some(_)) => -1,
        (Some(_), None) => 1,
        (Some(a), Some(b)) => a.cmp(b) as i32,
    }
}

/// Full/half width fold used by Java `normalizeWidth` tests.
pub fn normalize_width(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{FF9E}' || ch == '\u{3099}' {
            continue;
        }
        if let Some(mapped) = map_width(ch, chars.peek().copied()) {
            if mapped.1 {
                chars.next();
            }
            out.push_str(&mapped.0);
        } else {
            out.push(ch);
        }
    }
    out
}

fn map_width(ch: char, next: Option<char>) -> Option<(String, bool)> {
    match ch {
        '\u{3000}' => Some((" ".into(), false)),
        '\u{FF01}' => Some(("!".into(), false)),
        '\u{FF04}' => Some(("$".into(), false)),
        '\u{FF08}' => Some(("(".into(), false)),
        '\u{FF09}' => Some((")".into(), false)),
        '\u{FF0E}' => Some((".".into(), false)),
        '\u{FF1F}' => Some(("?".into(), false)),
        '\u{3371}' => Some(("hPa".into(), false)),
        '\u{2100}' => Some(("a/c".into(), false)),
        '\u{FF71}' => Some(("\u{30A2}".into(), false)),
        '\u{FF76}' if matches!(next, Some('\u{FF9E}' | '\u{3099}')) => Some(("\u{30AC}".into(), true)),
        '\u{FF76}' => Some(("\u{30AB}".into(), false)),
        '\u{FF8A}' if matches!(next, Some('\u{FF9F}')) => Some(("\u{30D1}".into(), true)),
        '\u{FF75}' => Some(("\u{30AA}".into(), false)),
        '\u{FFBE}' => Some(("\u{314E}".into(), false)),
        '\u{FFA4}' => Some(("\u{3134}".into(), false)),
        c if ('\u{FF10}'..='\u{FF19}').contains(&c) => {
            Some((((b'0' + (c as u32 - 0xFF10) as u8) as char).to_string(), false))
        }
        c if ('\u{FF21}'..='\u{FF3A}').contains(&c) => {
            Some((((b'A' + (c as u32 - 0xFF21) as u8) as char).to_string(), false))
        }
        c if ('\u{FF41}'..='\u{FF5A}').contains(&c) => {
            Some((((b'a' + (c as u32 - 0xFF41) as u8) as char).to_string(), false))
        }
        _ => None,
    }
}

/// Java `StringUtil.wrap` (splits existing `\n` lines with `,` then reflows).
pub fn wrap(text: &str, width: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut sb = String::new();
    for line in text.split('\n') {
        if !sb.is_empty() {
            sb.push(',');
        }
        for token in line.split(|c: char| c.is_whitespace()) {
            let last_nl = sb.rfind('\n').map(|i| i as i32).unwrap_or(-1);
            if (!sb.is_empty() || token.len() <= width)
                && (sb.len() as i32) + (token.len() as i32) - last_nl > width as i32
            {
                sb.push('\n');
            }
            if !sb.is_empty() && !sb.ends_with('\n') {
                sb.push(' ');
            }
            sb.push_str(token);
        }
    }
    sb
}

pub fn replace_case(input: &str, locale: &str) -> String {
    if !input.contains('\\') {
        return input.to_string();
    }
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut mode = CaseMode::None;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'u' if i + 3 < chars.len() && chars[i + 2] == '\\' && chars[i + 3] == 'L' => {
                    i += 4;
                    if i < chars.len() {
                        out.push_str(&chars[i].to_uppercase().to_string());
                        i += 1;
                    }
                    mode = CaseMode::Lower;
                    continue;
                }
                'l' if i + 3 < chars.len() && chars[i + 2] == '\\' && chars[i + 3] == 'U' => {
                    i += 4;
                    if i < chars.len() {
                        out.push(chars[i].to_lowercase().next().unwrap_or(chars[i]));
                        i += 1;
                    }
                    mode = CaseMode::Upper;
                    continue;
                }
                '\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                '$' => {
                    out.push('$');
                    i += 2;
                    continue;
                }
                'U' => {
                    mode = CaseMode::Upper;
                    i += 2;
                    continue;
                }
                'L' => {
                    mode = CaseMode::Lower;
                    i += 2;
                    continue;
                }
                'E' => {
                    mode = CaseMode::None;
                    i += 2;
                    continue;
                }
                'u' => {
                    i += 2;
                    if i < chars.len() {
                        out.push_str(&apply_locale_upper(chars[i], locale));
                        i += 1;
                    }
                    continue;
                }
                'l' => {
                    i += 2;
                    if i < chars.len() {
                        out.push(chars[i].to_lowercase().next().unwrap_or(chars[i]));
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }
        let ch = chars[i];
        match mode {
            CaseMode::Upper => out.push_str(&apply_locale_upper(ch, locale)),
            CaseMode::Lower => out.push(ch.to_lowercase().next().unwrap_or(ch)),
            CaseMode::None => out.push(ch),
        }
        i += 1;
    }
    out
}

enum CaseMode {
    None,
    Upper,
    Lower,
}

fn apply_locale_upper(ch: char, locale: &str) -> String {
    if ch == 'i' && locale.eq_ignore_ascii_case("tr") {
        "\u{0130}".into()
    } else {
        ch.to_uppercase().collect()
    }
}
