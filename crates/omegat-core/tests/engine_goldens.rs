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
fn glossary_tsv_and_query_match_java() {
    let spec = load_exported("glossary.json");
    let tab = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/java/src/test/resources/data/glossaries/test.tab");
    let raw = std::fs::read_to_string(&tab).unwrap();
    let parsed = omegat_core::glossary::parse_glossary(&raw);
    let expected = spec["entries"].as_array().unwrap();
    assert_eq!(parsed.len(), expected.len());
    for (got, exp) in parsed.iter().zip(expected) {
        assert_eq!(got.source, exp["source"].as_str().unwrap());
        assert_eq!(got.target, exp["target"].as_str().unwrap());
        assert_eq!(got.comment, exp["comment"].as_str().unwrap_or(""));
    }
    let mut entries = parsed;
    entries.push(omegat_core::glossary::GlossaryEntry {
        source: "running".into(),
        target: "courir".into(),
        comment: "verb".into(),
    });
    entries.push(omegat_core::glossary::GlossaryEntry {
        source: "Cat".into(),
        target: "chat".into(),
        comment: String::new(),
    });
    for case in spec["cases"].as_array().unwrap() {
        let segment = case["segment"].as_str().unwrap();
        let ignore_case = case["ignore_case"].as_bool().unwrap();
        let use_stem = case["use_stem"].as_bool().unwrap();
        let tgt = case["tgt_lang"].as_str().unwrap_or("fr");
        let expected: Vec<String> = case["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let hits = omegat_core::glossary::lookup_opts_lang(&entries, segment, ignore_case, use_stem, tgt);
        let got: Vec<String> = hits.into_iter().map(|h| h.target).collect();
        assert_eq!(got, expected, "glossary {segment:?} ignore_case={ignore_case} stem={use_stem}");
    }
}

#[test]
fn stats_bins_and_word_counts_match_java() {
    let spec = load_exported("stats.json");
    assert_eq!(spec["percent_exact_match"].as_i64(), Some(101));
    for case in spec["cases"].as_array().unwrap() {
        let percent = case["percent"].as_i64().unwrap() as i32;
        let bin = case["bin"].as_str().unwrap();
        assert_eq!(
            omegat_core::stats::bin_for_percent(percent),
            bin,
            "bin for {percent}"
        );
    }
    for case in spec["word_counts"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            omegat_core::stats::number_of_words(text) as i64,
            case["words"].as_i64().unwrap(),
            "words {text:?}"
        );
        let nosp = text.chars().filter(|c| !c.is_whitespace()).count() as i64;
        assert_eq!(nosp, case["chars_nosp"].as_i64().unwrap(), "chars_nosp {text:?}");
        assert_eq!(
            text.chars().count() as i64,
            case["chars"].as_i64().unwrap(),
            "chars {text:?}"
        );
    }
}
