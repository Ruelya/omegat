use regex::Regex;
use std::path::Path;

/// Minimal SRX-compatible sentence splitter.
pub fn split_sentences(text: &str, enabled: bool) -> Vec<String> {
    if !enabled {
        let t = text.trim();
        if t.is_empty() {
            return vec![];
        }
        return vec![text.to_string()];
    }
    let mut parts = Vec::new();
    let mut last = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for i in 0..chars.len() {
        let (_idx, ch) = chars[i];
        if matches!(ch, '.' | '!' | '?') {
            let next = chars.get(i + 1).map(|(_, c)| *c);
            let after_space = chars.get(i + 2).map(|(_, c)| *c);
            if next == Some(' ') || next == Some('\n') {
                if after_space.map(|c| c.is_uppercase() || c.is_numeric()).unwrap_or(true) {
                    let end = chars.get(i + 1).map(|(e, _)| *e).unwrap_or(text.len());
                    let chunk = text[last..end].trim();
                    if !chunk.is_empty() {
                        parts.push(chunk.to_string());
                    }
                    last = chars.get(i + 2).map(|(e, _)| *e).unwrap_or(end);
                }
            }
        }
    }
    let tail = text[last..].trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    if parts.is_empty() && !text.trim().is_empty() {
        parts.push(text.to_string());
    }
    parts
}

pub fn load_srx_rules(path: &Path) -> Option<Vec<(String, bool)>> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut rules = Vec::new();
    for cap in Regex::new("<rule break=\"(yes|no)\"")
        .unwrap()
        .captures_iter(&raw)
    {
        rules.push((cap[1].to_string(), &cap[1] == "yes"));
    }
    Some(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let parts = split_sentences("Hello world. How are you? Fine.", true);
        assert!(parts.len() >= 2);
    }

    #[test]
    fn paragraph_mode() {
        let parts = split_sentences("Hello world. How are you?", false);
        assert_eq!(parts.len(), 1);
    }
}
