//! Java `org.omegat.util.MagicComment`.

use std::collections::BTreeMap;
use std::path::Path;

pub fn parse(input: Option<&str>) -> BTreeMap<String, String> {
    let Some(s) = input else {
        return BTreeMap::new();
    };
    let Some(start) = s.find("-*-") else {
        return BTreeMap::new();
    };
    let rest = &s[start + 3..];
    let Some(end) = rest.find("-*-") else {
        return BTreeMap::new();
    };
    let body = rest[..end].trim();
    let mut out = BTreeMap::new();
    for part in body.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once(':') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

pub fn parse_file(path: &Path) -> BTreeMap<String, String> {
    let Ok(raw) = std::fs::read(path) else {
        return BTreeMap::new();
    };
    let text = if raw.starts_with(&[0xFF, 0xFE]) || raw.starts_with(&[0xFE, 0xFF]) {
        return BTreeMap::new();
    } else {
        String::from_utf8_lossy(&raw)
            .trim_start_matches('\u{feff}')
            .to_string()
    };
    let first = text.lines().next().unwrap_or("");
    parse(Some(first))
}
