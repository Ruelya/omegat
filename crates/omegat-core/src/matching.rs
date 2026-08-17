use crate::consts::{FUZZY_THRESHOLD, MAX_NEAR_STRINGS};
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
    pub comes_from: String,
    pub project: Option<String>,
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
        }
    }
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

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

pub fn token_score(a: &str, b: &str, lang: &str, use_stem: bool) -> i32 {
    let ta = tokenize(a, lang);
    let tb = tokenize(b, lang);
    if ta.is_empty() || tb.is_empty() {
        return 0;
    }
    let set_a: std::collections::HashSet<_> = ta
        .iter()
        .map(|t| if use_stem { t.stem.as_str() } else { t.text.as_str() })
        .collect();
    let set_b: std::collections::HashSet<_> = tb
        .iter()
        .map(|t| if use_stem { t.stem.as_str() } else { t.text.as_str() })
        .collect();
    let inter = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0
    } else {
        (inter * 100 / union) as i32
    }
}

pub fn score_pair(query: &str, candidate: &str, lang: &str) -> (i32, i32, i32) {
    let char_s = similarity(query, candidate);
    let stem_s = token_score(query, candidate, lang, true);
    let no_stem = token_score(query, candidate, lang, false);
    let adjusted = (char_s * 2 + stem_s + no_stem) / 4;
    (stem_s.max(char_s), no_stem.max(char_s), adjusted)
}

pub fn find_matches(
    query: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    lang: &str,
) -> Vec<NearString> {
    let mut out = Vec::new();
    for e in memory {
        if e.source.is_empty() {
            continue;
        }
        let (s, ns, adj) = score_pair(query, &e.source, lang);
        if s >= FUZZY_THRESHOLD || e.source == query {
            out.push(NearString {
                source: e.source.clone(),
                translation: e.translation.clone(),
                score: if e.source == query { 100 } else { s },
                score_no_stem: ns,
                adjusted_score: adj,
                comes_from: "MEMORY".into(),
                project: None,
            });
        }
    }
    for (e, origin) in extra {
        let (s, ns, adj) = score_pair(query, &e.source, lang);
        if s >= FUZZY_THRESHOLD {
            out.push(NearString {
                source: e.source.clone(),
                translation: e.translation.clone(),
                score: s,
                score_no_stem: ns,
                adjusted_score: adj,
                comes_from: origin.clone(),
                project: Some(origin.clone()),
            });
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out.truncate(MAX_NEAR_STRINGS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_is_100() {
        assert_eq!(similarity("abc", "abc"), 100);
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
        assert!(hits[0].score >= 30);
    }
}
