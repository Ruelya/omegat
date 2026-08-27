//! Java `org.omegat.util.EncodingDetector.detectHtmlEncoding`.

use std::path::Path;

/// Detect HTML/XML encoding: BOM, then `<?xml encoding`, then `<meta charset`.
/// `x-user-defined` maps to `windows-1252`. Missing declaration uses `default`
/// or UTF-8 (Java `checkEncodingOrDefault`).
pub fn detect_html_encoding(path: &Path, default: Option<&str>) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return default.unwrap_or("UTF-8").to_string(),
    };
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return "UTF-16BE".into();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return "UTF-16LE".into();
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return "UTF-8".into();
    }
    let n = bytes.len().min(8192);
    let head = String::from_utf8_lossy(&bytes[..n]);
    if let Some(enc) = sniff_xml_declaration(&head) {
        return normalize_charset(&enc);
    }
    if let Some(enc) = sniff_meta_charset(&head) {
        return normalize_charset(&enc);
    }
    match default {
        Some(d) if !d.is_empty() => normalize_charset(d),
        _ => "UTF-8".into(),
    }
}

fn normalize_charset(raw: &str) -> String {
    let e = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if e.eq_ignore_ascii_case("x-user-defined") {
        return "windows-1252".into();
    }
    if e.eq_ignore_ascii_case("utf8") || e.eq_ignore_ascii_case("utf-8") {
        return "UTF-8".into();
    }
    if e.eq_ignore_ascii_case("utf-16be") || e.eq_ignore_ascii_case("utf16-be") {
        return "UTF-16BE".into();
    }
    if e.eq_ignore_ascii_case("utf-16le") || e.eq_ignore_ascii_case("utf16-le") {
        return "UTF-16LE".into();
    }
    if e.eq_ignore_ascii_case("iso-8859-1") {
        return "ISO-8859-1".into();
    }
    if e.eq_ignore_ascii_case("windows-1252") || e.eq_ignore_ascii_case("cp1252") {
        return "windows-1252".into();
    }
    e.to_string()
}

fn sniff_xml_declaration(head: &str) -> Option<String> {
    let lower = head.to_ascii_lowercase();
    let start = lower.find("<?xml")?;
    let end = head[start..].find("?>")? + start;
    let decl = &head[start..end];
    charset_eq_value(decl, "encoding")
}

fn sniff_meta_charset(head: &str) -> Option<String> {
    if let Some(v) = charset_attr(head, "charset=") {
        return Some(v);
    }
    let lower = head.to_ascii_lowercase();
    if let Some(i) = lower.find("content-type") {
        let window = &head[i..head.len().min(i + 200)];
        if let Some(v) = charset_attr(window, "charset=") {
            return Some(v);
        }
    }
    if let Some(i) = lower.find("content=") {
        let window = &head[i..head.len().min(i + 200)];
        if let Some(v) = charset_attr(window, "charset=") {
            return Some(v);
        }
        // Java `file-HTMLUtils-x-user-defined-content.html`: `<meta content="x-user-defined"/>`
        if let Some(v) = take_token(window[8..].trim_start()) {
            return Some(v);
        }
    }
    None
}

fn charset_eq_value(fragment: &str, key: &str) -> Option<String> {
    let lower = fragment.to_ascii_lowercase();
    let key_l = key.to_ascii_lowercase();
    let i = lower.find(&key_l)?;
    let rest = fragment[i + key.len()..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    take_token(rest)
}

fn charset_attr(fragment: &str, key: &str) -> Option<String> {
    let lower = fragment.to_ascii_lowercase();
    let i = lower.find(&key.to_ascii_lowercase())?;
    let rest = fragment[i + key.len()..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    take_token(rest)
}

fn take_token(rest: &str) -> Option<String> {
    let rest = rest.trim_start_matches(['"', '\'', ' ', '\t']);
    let mut out = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
