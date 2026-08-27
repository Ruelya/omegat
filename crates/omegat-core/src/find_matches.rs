//! Java `org.omegat.core.statistics.FindMatches`.

use crate::consts::MAX_NEAR_STRINGS;
use crate::external_tm::penalty_from_origin;
use crate::levenshtein::token_similarity;
use crate::matching::NearString;
use crate::segment::{glue, load_srx_file, segment_with_srx, table_for};
use crate::tmx::{same_language, TmxEntry};
use crate::tokenize::{tokenize_verbatim, StemmingMode};
use std::collections::HashSet;

pub const PENALTY_FOR_FUZZY: i32 = 40;
const SUBSEGMENT_MATCH_THRESHOLD: i32 = 85;
const PENALTY_FOR_REMOVED: i32 = 5;
/// Java `Preferences.PENALTY_FOR_FOREIGN_MATCHES_DEFAULT`.
pub const PENALTY_FOR_FOREIGN_MATCHES_DEFAULT: i32 = 30;

#[derive(Debug, Clone, Default)]
pub struct FileTranslation {
    pub source: String,
    pub translation: String,
    pub file: String,
    pub fuzzy: bool,
}

#[derive(Debug, Clone)]
pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub memory: &'a [TmxEntry],
    pub extra: &'a [(TmxEntry, String)],
    pub files: &'a [FileTranslation],
    pub tokenizer: &'a str,
    pub source_lang: &'a str,
    pub target_lang: &'a str,
    pub threshold: i32,
    pub limit: usize,
    pub search_exactly_the_same: bool,
    pub run_separate_segment_match: bool,
    pub foreign_penalty: i32,
}

impl<'a> SearchRequest<'a> {
    pub fn new(
        query: &'a str,
        memory: &'a [TmxEntry],
        extra: &'a [(TmxEntry, String)],
        tokenizer: &'a str,
        source_lang: &'a str,
        target_lang: &'a str,
    ) -> Self {
        Self {
            query,
            memory,
            extra,
            files: &[],
            tokenizer,
            source_lang,
            target_lang,
            threshold: 30,
            limit: MAX_NEAR_STRINGS,
            search_exactly_the_same: false,
            run_separate_segment_match: false,
            foreign_penalty: PENALTY_FOR_FOREIGN_MATCHES_DEFAULT,
        }
    }
}

pub fn tokenize_stem(text: &str, tokenizer: &str) -> Vec<String> {
    crate::tokenize::tokenize_word_tokens(text, tokenizer, StemmingMode::Matching)
}

pub fn tokenize_no_stem(text: &str, tokenizer: &str) -> Vec<String> {
    crate::tokenize::tokenize_word_tokens(&text.to_lowercase(), tokenizer, StemmingMode::None)
}

pub fn tokenize_all(text: &str) -> Vec<String> {
    tokenize_verbatim(&text.to_lowercase())
}

fn scores_for(query: &str, candidate: &str, tokenizer: &str, penalty: i32, fuzzy: bool) -> (i32, i32, i32) {
    let mut stem = token_similarity(&tokenize_stem(query, tokenizer), &tokenize_stem(candidate, tokenizer));
    let mut no_stem = token_similarity(
        &tokenize_no_stem(query, tokenizer),
        &tokenize_no_stem(candidate, tokenizer),
    );
    let mut adj = token_similarity(&tokenize_all(query), &tokenize_all(candidate));
    stem -= penalty;
    no_stem -= penalty;
    adj -= penalty;
    if fuzzy {
        stem -= PENALTY_FOR_FUZZY;
        no_stem -= PENALTY_FOR_FUZZY;
        adj -= PENALTY_FOR_FUZZY;
    }
    (stem, no_stem, adj)
}

fn is_foreign(e: &TmxEntry, target_lang: &str) -> bool {
    e.props
        .iter()
        .any(|(k, v)| k == "foreignMatch" && v == "true")
        || e.props.iter().any(|(k, v)| {
            k == "targetLanguage" && !v.is_empty() && !same_language(v, target_lang)
        })
}

/// Search memory + extra TM + file translations + optional subsegment glue.
pub fn search(req: SearchRequest<'_>) -> Vec<NearString> {
    let mut result: Vec<NearString> = Vec::new();
    let query = req.query;

    let add = |result: &mut Vec<NearString>,
                   source: &str,
                   translation: &str,
                   comes_from: &str,
                   tmx_name: &str,
                   penalty: i32,
                   fuzzy: bool,
                   skip_exact: bool| {
        if translation.is_empty() {
            return;
        }
        if skip_exact && !req.search_exactly_the_same && source == query {
            return;
        }
        let (s, ns, adj) = scores_for(query, source, req.tokenizer, penalty, fuzzy);
        if req.threshold > 0 && s < req.threshold && ns < req.threshold && adj < req.threshold {
            return;
        }
        insert_near(
            result,
            query,
            source,
            translation,
            s,
            ns,
            adj,
            penalty,
            comes_from,
            tmx_name,
            req.limit,
            fuzzy,
        );
    };

    for e in req.memory {
        add(
            &mut result,
            &e.source,
            &e.translation,
            "MEMORY",
            "",
            e.penalty,
            false,
            true,
        );
    }
    for (e, origin) in req.extra {
        let mut penalty = e.penalty.max(penalty_from_origin(origin));
        if is_foreign(e, req.target_lang) {
            penalty += req.foreign_penalty;
        }
        add(
            &mut result,
            &e.source,
            &e.translation,
            "TM",
            origin,
            penalty,
            false,
            false,
        );
    }
    for f in req.files {
        if f.translation.is_empty() {
            continue;
        }
        add(
            &mut result,
            &f.source,
            &f.translation,
            "FILES",
            &f.file,
            0,
            f.fuzzy,
            false,
        );
    }

    if req.run_separate_segment_match {
        if let Some(doc) = load_srx_file(&crate::segment::default_srx_path()) {
            let table = table_for(&doc, req.source_lang);
            let seg = segment_with_srx(query, &table);
            if seg.sentences.len() > 1 {
                let mut fsrc = Vec::new();
                let mut ftrans = Vec::new();
                let mut names = HashSet::new();
                let mut max_penalty = 0;
                for onesrc in &seg.sentences {
                    let sub = search(SearchRequest {
                        query: onesrc,
                        memory: req.memory,
                        extra: req.extra,
                        files: req.files,
                        tokenizer: req.tokenizer,
                        source_lang: req.source_lang,
                        target_lang: req.target_lang,
                        threshold: req.threshold,
                        limit: 1,
                        search_exactly_the_same: true,
                        run_separate_segment_match: false,
                        foreign_penalty: req.foreign_penalty,
                    });
                    if let Some(hit) = sub.first() {
                        if hit.score >= SUBSEGMENT_MATCH_THRESHOLD {
                            fsrc.push(hit.source.clone());
                            ftrans.push(hit.translation.clone());
                            if let Some(p) = &hit.project {
                                if !p.is_empty() {
                                    names.insert(p.clone());
                                }
                            }
                            max_penalty = max_penalty.max(hit.penalty);
                        } else {
                            fsrc.push(String::new());
                            ftrans.push(String::new());
                        }
                    } else {
                        fsrc.push(String::new());
                        ftrans.push(String::new());
                    }
                }
                let glued_src = glue(req.source_lang, req.source_lang, &fsrc, &seg.spaces, &seg.brules);
                let glued_tr = glue(req.source_lang, req.target_lang, &ftrans, &seg.spaces, &seg.brules);
                if !glued_tr.trim().is_empty() {
                    add(
                        &mut result,
                        &glued_src,
                        &glued_tr,
                        "SUBSEGMENTS",
                        &names.into_iter().collect::<Vec<_>>().join(","),
                        max_penalty,
                        false,
                        false,
                    );
                }
            }
        }
    }
    result
}

pub fn search_simple(
    query: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    tokenizer: &str,
    source_lang: &str,
    target_lang: &str,
    threshold: i32,
    limit: usize,
    search_exactly_the_same: bool,
    run_separate_segment_match: bool,
    foreign_penalty: i32,
) -> Vec<NearString> {
    search(SearchRequest {
        query,
        memory,
        extra,
        files: &[],
        tokenizer,
        source_lang,
        target_lang,
        threshold,
        limit,
        search_exactly_the_same,
        run_separate_segment_match,
        foreign_penalty,
    })
}

fn insert_near(
    result: &mut Vec<NearString>,
    query: &str,
    source: &str,
    translation: &str,
    score: i32,
    score_no_stem: i32,
    adjusted: i32,
    penalty: i32,
    comes_from: &str,
    tmx_name: &str,
    limit: usize,
    fuzzy: bool,
) {
    for existing in result.iter_mut() {
        if existing.source == source && existing.translation == translation {
            if existing.project.as_deref().unwrap_or("").is_empty() && !tmx_name.is_empty() {
                existing.project = Some(tmx_name.to_string());
            }
            return;
        }
    }
    let mut pos = 0usize;
    for (i, st) in result.iter().enumerate() {
        if st.score < score {
            pos = i;
            break;
        }
        if st.score == score {
            if st.score_no_stem < score_no_stem {
                pos = i;
                break;
            }
            if st.score_no_stem == score_no_stem {
                if st.adjusted_score < adjusted {
                    pos = i;
                    break;
                }
                if score == 100 && st.source != query && source == query {
                    pos = i;
                    break;
                }
            }
        }
        pos = i + 1;
    }
    result.insert(
        pos,
        NearString {
            source: source.to_string(),
            translation: translation.to_string(),
            score,
            score_no_stem,
            adjusted_score: adjusted,
            penalty,
            comes_from: comes_from.to_string(),
            project: if tmx_name.is_empty() {
                None
            } else {
                Some(tmx_name.to_string())
            },
            fuzzy,
        },
    );
    if result.len() > limit.max(1) {
        result.pop();
    }
}

pub fn find_matches(
    query: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    lang: &str,
) -> Vec<NearString> {
    crate::matching::find_matches_threshold(query, memory, extra, lang, 30, MAX_NEAR_STRINGS)
}

#[allow(dead_code)]
const _: i32 = PENALTY_FOR_REMOVED;
