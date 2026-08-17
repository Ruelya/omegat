//! Engine goldens transcribed from Java Segmenter / tokenizer / FuzzyMatcher.

use omegat_core::matching::{find_matches_threshold, score_pair};
use omegat_core::segment::{load_srx_file, split_with_srx, table_for};
use omegat_core::tokenize::{stem, tokenize};
use omegat_core::tmx::TmxEntry;
use serde_json::Value;
use std::path::PathBuf;

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/goldens/engine")
}

#[test]
fn srx_sentences_match_java_list() {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(goldens().join("srx.json")).unwrap()).unwrap();
    let srx_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/srx/defaultRules.srx");
    let doc = load_srx_file(&srx_path).expect("defaultRules.srx");
    for case in spec["cases"].as_array().unwrap() {
        let lang = case["lang"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let expected: Vec<String> = case["sentences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let table = table_for(&doc, lang);
        let got = split_with_srx(input, &table);
        assert_eq!(got, expected, "srx {lang} {input:?}");
    }
}

#[test]
fn tokens_match_java_lists() {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(goldens().join("tokens.json")).unwrap()).unwrap();
    for case in spec["cases"].as_array().unwrap() {
        let lang = case["lang"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let expected: Vec<String> = case["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got: Vec<String> = tokenize(input, lang).into_iter().map(|t| t.text).collect();
        assert_eq!(got, expected, "tokens {lang} {input:?}");
        if let Some(stems) = case.get("stems") {
            let exp: Vec<String> = stems
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            let got: Vec<String> = tokenize(input, lang).into_iter().map(|t| t.stem).collect();
            assert_eq!(got, exp, "stems {lang} {input:?} stem(running)={}", stem("running", "en"));
        }
    }
}

#[test]
fn fuzzy_token_levenshtein_matches_java() {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(goldens().join("fuzzy.json")).unwrap()).unwrap();
    for case in spec["cases"].as_array().unwrap() {
        let lang = case["lang"].as_str().unwrap();
        let query = case["query"].as_str().unwrap();
        let candidate = case["candidate"].as_str().unwrap();
        let score = case["score"].as_i64().unwrap() as i32;
        let (s, _, _) = score_pair(query, candidate, lang);
        assert_eq!(s, score, "fuzzy {query:?} vs {candidate:?}");
        if score == 100 {
            let mem = vec![TmxEntry {
                source: candidate.into(),
                translation: "X".into(),
                ..Default::default()
            }];
            let hits = find_matches_threshold(query, &mem, &[], lang, 30, 5);
            assert_eq!(hits[0].score, 100);
        }
    }
}
