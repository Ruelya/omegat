//! Engine goldens must be written by `org.omegat.tools.ExportGoldens`.
//! Handwritten files do not count. Red is allowed until G1 is green.

use omegat_core::matching::{find_matches_threshold, score_pair};
use omegat_core::segment::{load_srx_file, split_with_srx, table_for};
use omegat_core::tmx::TmxEntry;
use serde_json::Value;
use std::path::PathBuf;

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/goldens/engine")
}

fn load_exported(name: &str) -> Value {
    let path = goldens().join(name);
    assert!(
        path.is_file(),
        "missing Java-exported engine golden {} — run reference/java ./gradlew exportGoldens",
        path.display()
    );
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        spec["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens"),
        "{} is not a Java export",
        path.display()
    );
    let java_test = spec["java_test"].as_str().unwrap_or("");
    assert!(
        java_test.starts_with("org.omegat.") && java_test.contains('#'),
        "fake java_test in {}: {java_test:?}",
        path.display()
    );
    spec
}

#[test]
fn srx_sentences_match_java_list() {
    let spec = load_exported("srx.json");
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
    let spec = load_exported("tokens.json");
    for case in spec["cases"].as_array().unwrap() {
        let lang = case["lang"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let expected: Vec<String> = case["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got: Vec<String> = omegat_core::tokenize::tokenize(input, lang)
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(got, expected, "tokens {lang} {input:?} tokenizer={}", case["tokenizer"]);
    }
}

#[test]
fn fuzzy_token_levenshtein_matches_java() {
    let spec = load_exported("fuzzy.json");
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
fn glossary_stats_require_java_export() {
    for name in ["glossary.json", "stats.json"] {
        let path = goldens().join(name);
        assert!(
            path.is_file(),
            "missing Java-exported {name} — G1 must export it with ExportGoldens"
        );
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            spec["exported_by"].as_str(),
            Some("org.omegat.tools.ExportGoldens"),
            "{name} is handwritten and voided"
        );
    }
}
