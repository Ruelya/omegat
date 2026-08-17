//! SRX 2.0 sentence segmentation, ported from Java `org.omegat.core.segmentation.Segmenter`.
//!
//! Rules are applied in reverse order. Break rules add positions; exception
//! (`break="no"`) rules remove them. After every rule, remaining exception
//! positions win. Language maps cascade (`DE.*` then `.*` → Default/Text/HTML).

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
    pub cascade: bool,
}

static DEFAULT_SRX: Lazy<SrxDocument> = Lazy::new(|| {
    load_srx_file(&default_srx_path()).unwrap_or_default()
});

static REGEX_CACHE: Lazy<Mutex<HashMap<String, Option<Regex>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static ANY_CHAR: Lazy<Regex> = Lazy::new(|| Regex::new("(?s).").expect("dotall dot"));

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
    let cascade = !raw.contains(r#"cascade="no""#);
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
    SrxDocument {
        languages,
        maps,
        cascade,
    }
}

/// Java `SRXManager.lookupRulesForLanguage`: every matching map, cascading.
pub fn table_for(doc: &SrxDocument, lang: &str) -> SrxTable {
    let tag = language_tag(lang);
    let mut rules = Vec::new();
    if doc.maps.is_empty() {
        if let Some(named) = doc.languages.get(lang) {
            rules.extend(named.rules.clone());
        }
        if let Some(def) = doc.languages.get("Default") {
            rules.extend(def.rules.clone());
        }
        return SrxTable { rules };
    }
    for (pat, name) in &doc.maps {
        if language_map_matches(pat, &tag) {
            if let Some(named) = doc.languages.get(name) {
                rules.extend(named.rules.clone());
            }
            if !doc.cascade {
                break;
            }
        }
    }
    SrxTable { rules }
}

fn language_tag(lang: &str) -> String {
    lang.trim().replace('_', "-")
}

fn language_map_matches(pattern: &str, lang: &str) -> bool {
    if let Some(re) = compile_java_regex(pattern, true) {
        return re.is_match(lang);
    }
    let stem = pattern.trim_end_matches(".*").trim_end_matches('*');
    lang.eq_ignore_ascii_case(stem) || lang.to_ascii_uppercase().starts_with(&stem.to_ascii_uppercase())
}

fn unescape_xml(s: &str) -> String {
    // Do not trim spaces: Text rules use ` +` (spaces after newline).
    let s = s.trim_matches(['\n', '\r']);
    html_escape::decode_html_entities(s).into_owned()
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

/// Java `Language.isSpaceDelimited`: only zh / ja / bo are not.
pub fn is_space_delimited(lang: &str) -> bool {
    let code = lang
        .split(['-', '_'])
        .next()
        .unwrap_or(lang)
        .to_ascii_uppercase();
    code != "ZH" && code != "JA" && code != "BO"
}

#[derive(Debug, Clone)]
pub struct Segmented {
    pub sentences: Vec<String>,
    pub spaces: Vec<String>,
    pub brules: Vec<SrxRule>,
}

/// Java `Segmenter.segment`: break, then record leading/trailing spaces.
pub fn segment_with_srx(text: &str, table: &SrxTable) -> Segmented {
    let (chunks, brules) = break_paragraph(text, table);
    let mut sentences = Vec::new();
    let mut spaces = Vec::new();
    for one in chunks {
        let bytes: Vec<char> = one.chars().collect();
        let mut b = 0usize;
        while b < bytes.len() && bytes[b].is_whitespace() {
            b += 1;
        }
        let mut e = bytes.len();
        while e > b && bytes[e - 1].is_whitespace() {
            e -= 1;
        }
        let leading: String = bytes[..b].iter().collect();
        let trailing: String = bytes[e..].iter().collect();
        let trimmed: String = bytes[b..e].iter().collect();
        sentences.push(trimmed);
        spaces.push(leading);
        spaces.push(trailing);
    }
    Segmented {
        sentences,
        spaces,
        brules,
    }
}

/// Java `Segmenter.glue`.
pub fn glue(
    source_lang: &str,
    target_lang: &str,
    sentences: &[String],
    spaces: &[String],
    brules: &[SrxRule],
) -> String {
    if sentences.is_empty() {
        return String::new();
    }
    let mut res = sentences[0].clone();
    for i in 1..sentences.len() {
        let mut sp = String::new();
        if 2 * i < spaces.len() {
            sp.push_str(&spaces[2 * i - 1]);
            sp.push_str(&spaces[2 * i]);
        }
        if !is_space_delimited(target_lang) {
            let rule = brules.get(i - 1);
            if !res.is_empty() {
                let last_char = res.chars().last().unwrap_or('\0');
                if let Some(caps) = LINE_BREAK_OR_TAB.captures(&sp) {
                    let left = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    if !left.is_empty() {
                        sp = sp[left.len()..].to_string();
                    }
                } else if last_char != '.' {
                    let before = rule.map(|r| r.before.as_str()).unwrap_or("");
                    let after = rule.map(|r| r.after.as_str()).unwrap_or("");
                    if !SPACY_REGEX.is_match(before) || !SPACY_REGEX.is_match(after) {
                        sp.clear();
                    }
                }
            }
        } else if !is_space_delimited(source_lang) && sp.is_empty() {
            sp.push(' ');
        }
        res.push_str(&sp);
        res.push_str(&sentences[i]);
    }
    res
}

static LINE_BREAK_OR_TAB: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^( *)[\r\n\t]").expect("line break or tab"));
static SPACY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"((\s|\\n|\\t|\\s)(\+|\*)?)+").expect("spacy"));

/// Java `Segmenter.breakParagraph` + trim from `segment`.
pub fn split_with_srx(text: &str, table: &SrxTable) -> Vec<String> {
    segment_with_srx(text, table)
        .sentences
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect()
}

fn break_paragraph(text: &str, table: &SrxTable) -> (Vec<String>, Vec<SrxRule>) {
    if text.is_empty() {
        return (vec![], vec![]);
    }
    let mut dontbreak: BTreeSet<usize> = BTreeSet::new();
    let mut breaks: std::collections::BTreeMap<usize, SrxRule> = std::collections::BTreeMap::new();
    for rule in table.rules.iter().rev() {
        let positions = get_breaks(text, rule);
        if rule.breaks {
            for p in &positions {
                dontbreak.remove(p);
                breaks.entry(*p).or_insert_with(|| rule.clone());
            }
        } else {
            for p in &positions {
                breaks.remove(p);
                dontbreak.insert(*p);
            }
        }
    }
    for p in dontbreak {
        breaks.remove(&p);
    }

    let mut segments = Vec::new();
    let mut brules = Vec::new();
    let mut prev = 0usize;
    for (pos, rule) in breaks {
        if pos > prev && pos <= text.len() && text.is_char_boundary(pos) {
            segments.push(text[prev..pos].to_string());
            brules.push(rule);
            prev = pos;
        }
    }
    let last = text[prev..].to_string();
    if last.trim().is_empty() && !segments.is_empty() {
        if let Some(prev_seg) = segments.last_mut() {
            prev_seg.push_str(&last);
        }
    } else {
        segments.push(last);
    }
    (segments, brules)
}

/// Java `Segmenter.getBreaks`: before/after regex `find`, after.start == before.end.
fn get_breaks(paragraph: &str, rule: &SrxRule) -> Vec<usize> {
    let before_re = if rule.before.is_empty() {
        Some(ANY_CHAR.clone())
    } else {
        compile_java_regex(&rule.before, false)
    };
    let after_re = if rule.after.is_empty() {
        None
    } else {
        compile_java_regex(&rule.after, false)
    };
    let Some(before_re) = before_re else {
        return vec![];
    };

    let after_starts: Option<Vec<usize>> = after_re.as_ref().map(|re| {
        re.find_iter(paragraph).map(|m| m.start()).collect()
    });
    if let Some(starts) = &after_starts {
        if starts.is_empty() {
            return vec![];
        }
    }

    // Java `Segmenter.getBreaks`: the after matcher is sequential and
    // returns early when it is exhausted (does not scan remaining befores).
    let mut res = Vec::new();
    match after_starts {
        None => {
            for m in before_re.find_iter(paragraph) {
                res.push(m.end());
            }
        }
        Some(starts) => {
            let mut after_idx = 0usize;
            for m in before_re.find_iter(paragraph) {
                let bbe = m.end();
                while after_idx < starts.len() && starts[after_idx] < bbe {
                    after_idx += 1;
                }
                if after_idx >= starts.len() {
                    return res;
                }
                if starts[after_idx] == bbe {
                    res.push(bbe);
                }
            }
        }
    }
    res
}

fn compile_java_regex(pattern: &str, case_insensitive: bool) -> Option<Regex> {
    let key = if case_insensitive {
        format!("i:{pattern}")
    } else {
        format!("s:{pattern}")
    };
    let mut cache = REGEX_CACHE.lock().ok()?;
    if let Some(hit) = cache.get(&key) {
        return hit.clone();
    }
    let rust_pat = java_pattern_to_rust(pattern, case_insensitive);
    let compiled = Regex::new(&rust_pat).ok();
    cache.insert(key, compiled.clone());
    compiled
}

/// Java `Rule.compilePattern`: DOTALL always; UNICODE_CASE when `(?i)` is set.
fn java_pattern_to_rust(pattern: &str, force_i: bool) -> String {
    let mut flags = String::from("(?s");
    if force_i || pattern.contains("(?i)") {
        flags.push('i');
    }
    flags.push(')');
    format!("{flags}{pattern}")
}

/// Kept for STATUS / older callers; now actually parses break attributes.
pub fn load_srx_rules(path: &Path) -> Option<Vec<(String, bool)>> {
    let doc = load_srx_file(path)?;
    let table = table_for(&doc, "Default");
    Some(
        table
            .rules
            .into_iter()
            .map(|r| (r.before, r.breaks))
            .collect(),
    )
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
            rules: vec![
                SrxRule {
                    breaks: false,
                    before: r"Mr\.".into(),
                    after: r"\s".into(),
                },
                SrxRule {
                    breaks: true,
                    before: r"[\.\?\!]+".into(),
                    after: r"\s".into(),
                },
            ],
        };
        let parts = split_with_srx("Mr. Smith went home. Next.", &table);
        assert!(parts.iter().any(|p| p.contains("Mr. Smith")), "{parts:?}");
        assert!(parts.len() >= 2, "{parts:?}");
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

    #[test]
    fn java_segmenter_br_test() {
        let path = default_srx_path();
        let doc = load_srx_file(&path).unwrap();
        let table = table_for(&doc, "en");
        let input = "<br7>\n\n<br5>\n\nother";
        let (chunks, _) = break_paragraph(input, &table);
        assert_eq!(chunks, vec!["<br7>", "\n\n<br5>", "\n\nother"], "{chunks:?}");
        let parts = split_with_srx(input, &table);
        assert_eq!(parts, vec!["<br7>", "<br5>", "other"]);
    }
}
