use crate::consts::MAX_NEAR_STRINGS;
use crate::tmx::TmxEntry;
use crate::tokenize::tokenize;
use omegat_ipc::MatchDto;

#[derive(Debug, Clone)]
pub struct NearString {
    pub source: String,
    pub translation: String,
    pub score: i32,
    pub score_no_stem: i32,
    pub adjusted_score: i32,
    pub penalty: i32,
    pub comes_from: String,
    pub project: Option<String>,
    /// Java `NearString.fuzzyMark` (PO `#, fuzzy` source translation).
    pub fuzzy: bool,
}

impl NearString {
    pub fn to_dto(&self) -> MatchDto {
        MatchDto {
            source: self.source.clone(),
            translation: self.translation.clone(),
            score: self.score,
            score_no_stem: self.score_no_stem,
            adjusted_score: self.adjusted_score,
            comes_from: self.comes_from.clone(),
            project: self.project.clone(),
            similarity: Vec::new(),
        }
    }
}

pub use crate::levenshtein::{
    char_levenshtein as levenshtein, token_levenshtein, token_similarity,
};

pub fn similarity(a: &str, b: &str) -> i32 {
    if a == b {
        return 100;
    }
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let dist = levenshtein(a, b);
    let max = a.chars().count().max(b.chars().count());
    ((max - dist) * 100 / max) as i32
}

fn tokens(text: &str, lang: &str, use_stem: bool) -> Vec<String> {
    tokenize(text, lang)
        .into_iter()
        .map(|t| if use_stem { t.stem } else { t.text })
        .collect()
}

pub fn score_pair(query: &str, candidate: &str, lang: &str) -> (i32, i32, i32) {
    if query == candidate {
        return (100, 100, 100);
    }
    let stem_s = token_similarity(&tokens(query, lang, true), &tokens(candidate, lang, true));
    let no_stem = token_similarity(&tokens(query, lang, false), &tokens(candidate, lang, false));
    let adjusted = (stem_s * 2 + no_stem) / 3;
    (stem_s, no_stem, adjusted)
}

pub fn find_matches(
    query: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    lang: &str,
) -> Vec<NearString> {
    find_matches_threshold(query, memory, extra, lang, 30, MAX_NEAR_STRINGS)
}

pub fn find_matches_threshold(
    query: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    lang: &str,
    threshold: i32,
    limit: usize,
) -> Vec<NearString> {
    let mut out = Vec::new();
    for e in memory {
        if e.source.is_empty() {
            continue;
        }
        let (s, ns, adj) = score_pair(query, &e.source, lang);
        if s >= threshold || e.source == query {
            out.push(NearString {
                source: e.source.clone(),
                translation: e.translation.clone(),
                score: if e.source == query { 100 } else { s },
                score_no_stem: ns,
                adjusted_score: adj,
                penalty: 0,
                comes_from: "MEMORY".into(),
                project: None,
                fuzzy: false,
            });
        }
    }
    for (e, origin) in extra {
        let (s, ns, adj) = score_pair(query, &e.source, lang);
        if s >= threshold {
            let penalty = if e.penalty > 0 {
                e.penalty
            } else {
                e.note
                    .as_deref()
                    .and_then(|n| n.strip_prefix("penalty:")?.parse::<i32>().ok())
                    .unwrap_or(0)
            };
            let score = (s - penalty).max(0);
            let comes = if origin.contains("mt/") || origin.contains("/mt/") {
                "MT".to_string()
            } else if origin.contains("auto") {
                "TM".to_string()
            } else {
                origin.clone()
            };
            out.push(NearString {
                source: e.source.clone(),
                translation: e.translation.clone(),
                score,
                score_no_stem: ns,
                adjusted_score: (adj - penalty).max(0),
                penalty,
                comes_from: comes,
                project: Some(origin.clone()),
                fuzzy: false,
            });
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out.truncate(limit.max(1));
    out
}

/// Java `StringData.UNIQ` / `StringData.PAIR` (not a 0/1 contains bitmap).
pub const SIM_UNIQ: u8 = 0x01;
pub const SIM_PAIR: u8 = 0x02;

/// Token-level alignment bytes for match highlighting (Java `FuzzyMatcher.buildSimilarityData`).
pub fn similarity_data(source: &str, r#match: &str, lang: &str) -> Vec<u8> {
    let src = tokens(source, lang, false);
    let cand = tokens(r#match, lang, false);
    let mut result = vec![0u8; cand.len()];
    let mut leftfound = true;
    for i in 0..cand.len() {
        let rightfound = i + 1 == cand.len() || src.iter().any(|t| t == &cand[i + 1]);
        let found = src.iter().any(|t| t == &cand[i]);
        if found && (!leftfound || !rightfound) {
            result[i] = SIM_PAIR;
        } else if !found {
            result[i] = SIM_UNIQ;
        }
        leftfound = found;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_is_100() {
        assert_eq!(score_pair("abc", "abc", "en").0, 100);
    }

    #[test]
    fn token_edit_hello_word() {
        let (s, _, _) = score_pair("Hello world", "Hello word", "en");
        assert_eq!(s, 50);
    }

    #[test]
    fn finds_near() {
        let mem = vec![TmxEntry {
            source: "Hello world".into(),
            translation: "Bonjour le monde".into(),
            ..Default::default()
        }];
        let hits = find_matches("Hello word", &mem, &[], "en");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].score, 50);
    }

    #[test]
    fn penalty_folder_lowers_score() {
        let extra = vec![(
            TmxEntry {
                source: "Hello world".into(),
                translation: "X".into(),
                note: Some("penalty:10".into()),
                ..Default::default()
            },
            "penalty-010/ref.tmx".into(),
        )];
        let hits = find_matches_threshold("Hello world", &[], &extra, "en", 30, 5);
        assert_eq!(hits[0].score, 90);
    }
}
