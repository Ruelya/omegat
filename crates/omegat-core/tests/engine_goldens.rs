//! Engine goldens transcribed from Java Segmenter / tokenizer / FuzzyMatcher.

use omegat_core::glossary::{lookup_opts, parse_glossary};
use omegat_core::matching::{find_matches_threshold, score_pair};
use omegat_core::segment::{load_srx_file, split_with_srx, table_for};
use omegat_core::session::Entry;
use omegat_core::stats::{compute, render};
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

#[test]
fn glossary_options_change_hits() {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(goldens().join("glossary.json")).unwrap()).unwrap();
    let entries = parse_glossary(spec["entries"].as_str().unwrap());
    for case in spec["cases"].as_array().unwrap() {
        let hits = lookup_opts(
            &entries,
            case["segment"].as_str().unwrap(),
            case["ignore_case"].as_bool().unwrap(),
            case["use_stem"].as_bool().unwrap(),
        );
        let got: Vec<String> = hits.into_iter().map(|h| h.target).collect();
        let exp: Vec<String> = case["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(got, exp, "glossary {:?}", case);
    }
}

#[test]
fn stats_formats_match_java_shape() {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(goldens().join("stats.json")).unwrap()).unwrap();
    let entries: Vec<Entry> = spec["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| Entry {
            file: e["file"].as_str().unwrap().into(),
            id: e["source"].as_str().unwrap().into(),
            source: e["source"].as_str().unwrap().into(),
            translation: e["translation"].as_str().unwrap().into(),
            note: String::new(),
            comment: String::new(),
            default_translation: true,
            revision: 1,
            from_tm_exact: e["exact"].as_bool().unwrap_or(false),
            properties: vec![],
        })
        .collect();
    let s = compute(&entries, "en", "fr");
    let exp = &spec["expect"];
    assert_eq!(s.total.segments, exp["total_segments"].as_u64().unwrap() as usize);
    assert_eq!(s.remaining.segments, exp["remaining_segments"].as_u64().unwrap() as usize);
    assert_eq!(s.unique.segments, exp["unique_segments"].as_u64().unwrap() as usize);
    assert_eq!(
        s.unique_remaining.segments,
        exp["unique_remaining_segments"].as_u64().unwrap() as usize
    );
    let text = render(&s, "text");
    for needle in exp["text_contains"].as_array().unwrap() {
        assert!(text.contains(needle.as_str().unwrap()), "missing {needle} in {text}");
    }
    let xml = render(&s, "xml");
    for needle in exp["xml_contains"].as_array().unwrap() {
        assert!(xml.contains(needle.as_str().unwrap()), "missing {needle} in {xml}");
    }
    let json = render(&s, "json");
    assert!(json.contains("\"unique-remaining\""));
}
