//! SRX 2.0 sentence segmentation. Rules load from OmegaT `defaultRules.srx`.
//! Lookahead is implemented by scanning candidate break points (not ICU).

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SrxRule {
    pub breaks: bool,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Default)]
pub struct SrxTable {
    pub rules: Vec<SrxRule>,
}

#[derive(Debug, Clone, Default)]
pub struct SrxDocument {
    pub languages: HashMap<String, SrxTable>,
    pub maps: Vec<(String, String)>,
}

static DEFAULT_SRX: Lazy<SrxDocument> = Lazy::new(|| {
    load_srx_file(&default_srx_path()).unwrap_or_default()
});

pub fn default_srx_path() -> PathBuf {
    if let Ok(p) = std::env::var("OMEGAT_SRX") {
        return PathBuf::from(p);
    }
    let candidates = [
        PathBuf::from("fixtures/srx/defaultRules.srx"),
        PathBuf::from("../fixtures/srx/defaultRules.srx"),
        PathBuf::from("../../fixtures/srx/defaultRules.srx"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/srx/defaultRules.srx"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("fixtures/srx/defaultRules.srx"))
}

pub fn load_srx_file(path: &Path) -> Option<SrxDocument> {
    let raw = std::fs::read_to_string(path).ok()?;
    Some(parse_srx_document(&raw))
}

pub fn parse_srx(raw: &str, language_rule: &str) -> SrxTable {
    let doc = parse_srx_document(raw);
    table_for(&doc, language_rule)
}

pub fn parse_srx_document(raw: &str) -> SrxDocument {
    let mut languages = HashMap::new();
    let mut maps = Vec::new();
    let rule_re = Regex::new(
        r#"(?s)<rule\s+break="(yes|no)"\s*>.*?<beforebreak>(.*?)</beforebreak>\s*<afterbreak>(.*?)</afterbreak>"#,
    )
    .unwrap();
    let lang_re = Regex::new(r#"(?s)<languagerule\s+languagerulename="([^"]+)">(.*?)</languagerule>"#)
        .unwrap();
    for cap in lang_re.captures_iter(raw) {
        let name = cap[1].to_string();
        let body = &cap[2];
        let mut rules = Vec::new();
        for rc in rule_re.captures_iter(body) {
            rules.push(SrxRule {
                breaks: &rc[1] == "yes",
                before: unescape_xml(&rc[2]),
                after: unescape_xml(&rc[3]),
            });
        }
        languages.insert(name, SrxTable { rules });
    }
    let map_re = Regex::new(
        r#"<languagemap\s+languagepattern="([^"]+)"\s+languagerulename="([^"]+)""#,
    )
    .unwrap();
    for cap in map_re.captures_iter(raw) {
        maps.push((cap[1].to_string(), cap[2].to_string()));
    }
    SrxDocument { languages, maps }
}

fn table_for(doc: &SrxDocument, lang: &str) -> SrxTable {
    let mut rules = Vec::new();
    let key = lang_rule_name(doc, lang);
    if let Some(named) = doc.languages.get(&key) {
        rules.extend(named.rules.clone());
    } else if let Some(named) = doc.languages.get(lang) {
        rules.extend(named.rules.clone());
    }
    if let Some(def) = doc.languages.get("Default") {
        rules.extend(def.rules.clone());
    }
    SrxTable { rules }
}

fn lang_rule_name(doc: &SrxDocument, lang: &str) -> String {
    let upper = lang.to_ascii_uppercase();
    for (pat, name) in &doc.maps {
        if pat == ".*" {
            continue;
        }
        let stem = pat.trim_end_matches(".*").trim_end_matches('*');
        if upper.starts_with(stem) || upper == stem {
            return name.clone();
        }
    }
    match crate::tokenize::lang_base(lang) {
        "en" => "English".into(),
        "de" => "German".into(),
        "fr" => "French".into(),
        "es" => "Spanish".into(),
        "it" => "Italian".into(),
        "ja" => "Japanese".into(),
        "zh" => "Chinese".into(),
        "nl" => "Dutch".into(),
        "pl" => "Polish".into(),
        "ru" => "Russian".into(),
        "sv" => "Swedish".into(),
        "sk" => "Slovak".into(),
        "cs" => "Czech".into(),
        "ca" => "Catalan".into(),
        "fi" => "Finnish".into(),
        _ => "Default".into(),
    }
}

fn unescape_xml(s: &str) -> String {
    html_escape::decode_html_entities(s.trim()).into_owned()
}

pub fn split_sentences(text: &str, enabled: bool) -> Vec<String> {
    split_sentences_lang(text, enabled, "en", None)
}

pub fn split_sentences_lang(
    text: &str,
    enabled: bool,
    lang: &str,
    table: Option<&SrxTable>,
) -> Vec<String> {
    if !enabled {
        let t = text.trim();
        if t.is_empty() {
            return vec![];
        }
        return vec![text.to_string()];
    }
    if let Some(t) = table {
        return split_with_srx(text, t);
    }
    if let Ok(path) = std::env::var("OMEGAT_SRX") {
        if let Some(doc) = load_srx_file(Path::new(&path)) {
            return split_with_srx(text, &table_for(&doc, lang));
        }
    }
    split_with_srx(text, &table_for(&DEFAULT_SRX, lang))
}

pub fn split_with_srx(text: &str, table: &SrxTable) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![];
    }
    let mut breaks = vec![0usize];
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for i in 0..chars.len() {
        let (idx, ch) = chars[i];
        if !matches!(ch, '.' | '!' | '?' | '。' | '！' | '？' | '\n' | '"' | '”') {
            continue;
        }
        let next_idx = chars.get(i + 1).map(|(e, _)| *e).unwrap_or(text.len());
        let before = &text[..=idx];
        let after = &text[next_idx..];
        if !should_break(before, after, table) {
            continue;
        }
        let mut end = next_idx;
        while end < text.len() && text[end..].chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
            end += text[end..].chars().next().unwrap().len_utf8();
        }
        if end > *breaks.last().unwrap() {
            breaks.push(end);
        }
    }
    if *breaks.last().unwrap() != text.len() {
        breaks.push(text.len());
    }
    let mut parts = Vec::new();
    for w in breaks.windows(2) {
        let chunk = text[w[0]..w[1]].trim();
        if !chunk.is_empty() {
            parts.push(chunk.to_string());
        }
    }
    if parts.is_empty() {
        parts.push(text.to_string());
    }
    parts
}

fn should_break(before: &str, after: &str, table: &SrxTable) -> bool {
    for rule in &table.rules {
        if matches_rule(&rule.before, before, true) && matches_rule(&rule.after, after, false) {
            return rule.breaks;
        }
    }
    matches!(
        before.chars().last(),
        Some('.' | '!' | '?' | '。' | '！' | '？' | '\n')
    ) && after
        .trim_start()
        .chars()
        .next()
        .map(|c| c.is_uppercase() || c.is_numeric() || c == '\n')
        .unwrap_or(true)
}

fn matches_rule(pattern: &str, hay: &str, from_end: bool) -> bool {
    if pattern.is_empty() || pattern == "." {
        return true;
    }
    let window = if from_end {
        let start = hay.len().saturating_sub(160);
        &hay[start..]
    } else {
        let end = hay.len().min(160);
        &hay[..end]
    };
    let rust_pat = icuish_to_rust(pattern);
    let anchored = if from_end {
        format!("(?s)(?:{rust_pat})$")
    } else {
        format!("(?s)^(?:{rust_pat})")
    };
    if let Ok(re) = Regex::new(&anchored) {
        return re.is_match(window);
    }
    if from_end {
        hay.ends_with(pattern)
    } else {
        hay.starts_with(pattern)
    }
}

/// Best-effort ICU → Rust regex for the SRX patterns OmegaT ships.
fn icuish_to_rust(pattern: &str) -> String {
    pattern
        .replace(r"\p{Lu}", r"\p{Lu}")
        .replace(r"\p{Nd}", r"\d")
        .replace(r"\P{Lu}", r"[^A-ZÀ-ÖØ-Þ]")
        .replace("(?i)", "(?i)")
}

/// Kept for STATUS / older callers; now actually parses break attributes.
pub fn load_srx_rules(path: &Path) -> Option<Vec<(String, bool)>> {
    let doc = load_srx_file(path)?;
    let table = table_for(&doc, "Default");
    Some(table.rules.into_iter().map(|r| (r.before, r.breaks)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let parts = split_sentences("Hello world. How are you? Fine.", true);
        assert!(parts.len() >= 2, "{parts:?}");
    }

    #[test]
    fn paragraph_mode() {
        let parts = split_sentences("Hello world. How are you?", false);
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn srx_file_loads() {
        let path = default_srx_path();
        assert!(path.exists(), "{}", path.display());
        let doc = load_srx_file(&path).expect("srx");
        assert!(doc.languages.contains_key("Default"));
        assert!(doc.languages.contains_key("English"));
        let table = table_for(&doc, "en");
        assert!(!table.rules.is_empty(), "defaultRules.srx must yield rules");
    }

    #[test]
    fn mr_does_not_break_when_rule_present() {
        let table = SrxTable {
            rules: vec![SrxRule {
                breaks: false,
                before: r"Mr\.".into(),
                after: r"\s".into(),
            }],
        };
        let parts = split_with_srx("Mr. Smith went home. Next.", &table);
        assert!(parts.iter().any(|p| p.contains("Mr. Smith")), "{parts:?}");
    }

    #[test]
    fn english_rules_keep_abbreviation() {
        let path = default_srx_path();
        let doc = load_srx_file(&path).unwrap();
        let table = table_for(&doc, "en");
        let parts = split_with_srx("Mr. Smith went home. Next sentence.", &table);
        assert!(parts.iter().any(|p| p.contains("Mr. Smith")), "{parts:?}");
        assert!(parts.len() >= 2, "{parts:?}");
    }
}
