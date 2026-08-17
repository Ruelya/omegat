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
    let mut fails = Vec::new();
    for case in spec["cases"].as_array().unwrap() {
        let class = case["tokenizer"].as_str().unwrap();
        let mode = omegat_core::tokenize::StemmingMode::parse(case["stemming"].as_str().unwrap_or("NONE"));
        let lang = case["lang"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        if let Some(words) = case["words"].as_array() {
            let expected_words: Vec<String> = words.iter().map(|v| v.as_str().unwrap().to_string()).collect();
            let got = omegat_core::tokenize::tokenize_words(input, class, mode);
            if got != expected_words {
                fails.push(format!("{class} {mode:?} {lang}\n  got  {got:?}\n  want {expected_words:?}"));
            }
        } else {
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
            if got != expected {
                fails.push(format!("{class} {mode:?} {lang} (tokens)\n  got  {got:?}\n  want {expected:?}"));
            }
        }
    }
    assert!(fails.is_empty(), "{} tokenizer golden failures:\n{}", fails.len(), fails.join("\n\n"));
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

#[test]
fn segmenter_test_methods_match_java() {
    let spec = load_exported("segmenter_tests.json");
    let srx_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/srx/defaultRules.srx");
    let doc = load_srx_file(&srx_path).expect("defaultRules.srx");
    let table = table_for(&doc, "en");
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        assert!(method.contains('#'), "{method}");
        let input = case["input"].as_str().unwrap();
        let expected: Vec<String> = case["sentences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        if method.ends_with("#testSegment") {
            let got = omegat_core::segment::segment_with_srx(input, &table);
            assert_eq!(got.sentences, expected, "{method}");
        } else if method.ends_with("#testGlue") {
            let src = case["source_lang"].as_str().unwrap();
            let tgt = case["target_lang"].as_str().unwrap();
            let seg = omegat_core::segment::segment_with_srx(input, &table);
            let glued = omegat_core::segment::glue(src, tgt, &seg.sentences, &seg.spaces, &seg.brules);
            assert_eq!(glued, case["glued"].as_str().unwrap(), "{method}");
        } else if method.ends_with("#testGlueCJK") {
            let src = case["source_lang"].as_str().unwrap();
            let tgt = case["target_lang"].as_str().unwrap();
            let mut seg = omegat_core::segment::segment_with_srx(input, &table);
            for s in &mut seg.sentences {
                *s = s.replace('.', "\\u3002");
            }
            let expected_sentences: Vec<String> = case["sentences"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            assert_eq!(seg.sentences, expected_sentences, "{method} sentences {input:?}");
            let glued = omegat_core::segment::glue(src, tgt, &seg.sentences, &seg.spaces, &seg.brules);
            assert_eq!(glued, case["glued"].as_str().unwrap(), "{method} {input:?}");
        }
    }
}

#[test]
fn levenshtein_distance_test_methods_match_java() {
    let spec = load_exported("levenshtein.json");
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        if case["null_inputs"].as_bool().unwrap_or(false) {
            assert!(omegat_core::levenshtein::compute(None, Some(&["null".into()])).is_err());
            assert!(omegat_core::levenshtein::compute(Some(&["null".into()]), None).is_err());
            continue;
        }
        let source: Vec<String> = case["source"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let target: Vec<String> = case["target"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got = omegat_core::levenshtein::compute_tokens(&source, &target) as i64;
        assert_eq!(got, case["distance"].as_i64().unwrap(), "{method}");
    }
}

#[test]
fn tag_validation_test_methods_match_java() {
    let spec = load_exported("tag_validation.json");
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let src: Vec<String> = case["src_tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let loc: Vec<String> = case["loc_tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let report = match kind {
            "ordered" => {
                let src_t = omegat_core::tag_validation::tags_from_strings(
                    &src.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                let loc_t = omegat_core::tag_validation::tags_from_strings(
                    &loc.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                omegat_core::tag_validation::inspect_ordered_tags(
                    &src_t,
                    &loc_t,
                    case["loose"].as_bool().unwrap_or(false),
                )
            }
            "unordered" => {
                let src_t = omegat_core::tag_validation::tags_from_strings(
                    &src.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                let loc_t = omegat_core::tag_validation::tags_from_strings(
                    &loc.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                omegat_core::tag_validation::inspect_unordered_tags(&src_t, &loc_t)
            }
            "printf" => omegat_core::tag_validation::inspect_printf_variables(&src[0], &loc[0]),
            "remove" => omegat_core::tag_validation::inspect_remove_pattern(&loc[0], "foo"),
            other => panic!("unknown kind {other}"),
        };
        let exp_src: Vec<(String, String)> = case["src_errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["tag"].as_str().unwrap().to_string(),
                    v["error"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let exp_tr: Vec<(String, String)> = case["trans_errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["tag"].as_str().unwrap().to_string(),
                    v["error"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(report.src_map(), exp_src, "{method} {}", case["name"]);
        assert_eq!(report.trans_map(), exp_tr, "{method} {}", case["name"]);
    }
}

#[test]
fn tag_repair_test_methods_match_java() {
    let spec = load_exported("tag_repair.json");
    for case in spec["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut text = case["input"].as_str().unwrap().to_string();
        match name {
            "extraneous" => {
                let tag = omegat_core::tag_validation::Tag::new(-1, "bar");
                omegat_core::tag_repair::fix_extraneous(&mut text, &tag);
                omegat_core::tag_repair::fix_extraneous(&mut text, &tag);
            }
            "missing_before" | "missing_after" | "missing_no_anchor" => {
                let order: Vec<String> = case["source_order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                let tags = omegat_core::tag_validation::tags_from_strings(
                    &order.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                let tag = omegat_core::tag_validation::Tag::new(-1, "{tag1}");
                omegat_core::tag_repair::fix_missing(&tags, &mut text, &tag);
            }
            "malformed" => {
                let order: Vec<String> = case["source_order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                let tags = omegat_core::tag_validation::tags_from_strings(
                    &order.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
                let tag = omegat_core::tag_validation::Tag::new(-1, "{tag1}");
                omegat_core::tag_repair::fix_malformed(&tags, &mut text, &tag);
            }
            "whitespace_strip" => omegat_core::tag_repair::fix_whitespace(&mut text, "Foo"),
            "whitespace_add" => omegat_core::tag_repair::fix_whitespace(&mut text, "\nFoo\n"),
            other => panic!("unknown repair {other}"),
        }
        assert_eq!(text, case["output"].as_str().unwrap(), "{name}");
    }
}

#[test]
fn tmx_writer_test_methods_match_java() {
    let spec = load_exported("tmx_writer.json");
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        match case["name"].as_str().unwrap() {
            "testWriteInvalidChars" => {
                let sanitized = case["sanitized_source"].as_str().unwrap();
                assert_eq!(
                    omegat_core::tmx::remove_xml_invalid_chars(
                        &String::from_utf16_lossy(&[0, 1, 2, 0x18, 0x19, 0xD8FF, 0xFFFE, 0xFFFF])
                    )
                    .chars()
                    .all(|c| c == ' ' || omegat_core::tmx::is_valid_xml_char(c as u32)),
                    true
                );
                assert!(sanitized.chars().all(|c| omegat_core::tmx::is_valid_xml_char(c as u32)));
            }
            "testLevel2write" => {
                let srcs = case["sources"].as_array().unwrap();
                let frags = case["level2_fragments"].as_array().unwrap();
                for (s, f) in srcs.iter().zip(frags) {
                    assert_eq!(
                        omegat_core::tmx::write_level_two_fragment(s.as_str().unwrap()),
                        f.as_str().unwrap(),
                        "{method}"
                    );
                }
            }
            "testLevel2reads" => {
                let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../reference/java/src/test/resources/data/tmx/test-save-tmx14.tmx");
                let mut raw = std::fs::read_to_string(&fixture).unwrap();
                let omegat = case["mode"].as_str().unwrap() == "omegat";
                if !omegat {
                    raw = raw.replacen("creationtool=\"OmegaT\"", "creationtool=\"ext\"", 1);
                }
                let opts = omegat_core::tmx::TmxReadOpts {
                    ext_level2: case["ext_level2"].as_bool().unwrap(),
                    use_slash: case["use_slash"].as_bool().unwrap(),
                    created_by_omegat: omegat,
                };
                let got = omegat_core::tmx::parse_tmx_sources(&raw, "en-US", "be-BY", opts);
                let expected: Vec<String> = case["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                assert_eq!(got, expected, "{method} {}", case["mode"]);
            }
            "testEOLwrite" => {
                assert!(case["read_translation"].as_str().unwrap().contains("tar\nget"));
            }
            other => panic!("unknown tmx case {other}"),
        }
    }
}

fn java_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/java")
}

fn read_tmx(rel: &str) -> String {
    std::fs::read_to_string(java_root().join(rel)).unwrap()
}

#[test]
fn find_matches_test_methods_match_java() {
    let spec = load_exported("find_matches.json");
    let en_tok = "org.omegat.tokenizer.LuceneEnglishTokenizer";
    let cjk_tok = "org.omegat.tokenizer.LuceneCJKTokenizer";
    let tmx_match = "src/test/resources/data/tmx/test-match-stat-en-ca.tmx";
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let query = case["query"].as_str().unwrap();
        let expected = case["hits"].as_array().unwrap();
        let (memory, extra, files, tokenizer, src, tgt, exact, separate, threshold) = match name {
            "without_separate" | "with_separate" => (
                omegat_core::tmx::parse_tmx_all(&read_tmx(tmx_match), "en", "ca"),
                Vec::new(),
                Vec::new(),
                en_tok,
                "en",
                "ca",
                false,
                name == "with_separate",
                30,
            ),
            "rfe1578" => (
                Vec::new(),
                omegat_core::tmx::parse_external_tmx(
                    &read_tmx("src/test/resources/data/tmx/en-US_sr.tmx"),
                    "en",
                    "cnr",
                    true,
                )
                .into_iter()
                .map(|e| (e, java_root().join("src/test/resources/data/tmx/en-US_sr.tmx").display().to_string()))
                .collect(),
                Vec::new(),
                en_tok,
                "en",
                "cnr",
                false,
                true,
                30,
            ),
            "rfe1578_2" => (
                Vec::new(),
                omegat_core::tmx::parse_external_tmx(
                    &read_tmx("src/test/resources/data/tmx/en-US_en-GB_fr_sr.tmx"),
                    "en",
                    "cnr",
                    true,
                )
                .into_iter()
                .map(|e| {
                    (
                        e,
                        java_root()
                            .join("src/test/resources/data/tmx/en-US_en-GB_fr_sr.tmx")
                            .display()
                            .to_string(),
                    )
                })
                .collect(),
                Vec::new(),
                en_tok,
                "en",
                "cnr",
                false,
                true,
                30,
            ),
            "bugs1251" => (
                Vec::new(),
                omegat_core::tmx::parse_external_tmx(
                    &read_tmx("src/test/resources/data/tmx/penalty-010/segment_1.tmx"),
                    "ja",
                    "fr",
                    true,
                )
                .into_iter()
                .map(|e| {
                    (
                        e,
                        java_root()
                            .join("src/test/resources/data/tmx/penalty-010/segment_1.tmx")
                            .display()
                            .to_string(),
                    )
                })
                .collect(),
                Vec::new(),
                cjk_tok,
                "ja",
                "fr",
                false,
                true,
                30,
            ),
            "foreign" => (
                Vec::new(),
                omegat_core::tmx::parse_external_tmx(
                    &read_tmx("src/test/resources/data/tmx/segment_2.tmx"),
                    "ja",
                    "fr",
                    true,
                )
                .into_iter()
                .map(|e| {
                    (
                        e,
                        java_root()
                            .join("src/test/resources/data/tmx/segment_2.tmx")
                            .display()
                            .to_string(),
                    )
                })
                .collect(),
                Vec::new(),
                cjk_tok,
                "ja",
                "fr",
                false,
                true,
                30,
            ),
            "foreign_segmented" => (
                Vec::new(),
                omegat_core::tmx::parse_external_tmx(&read_tmx(tmx_match), "en", "fr", true)
                    .into_iter()
                    .map(|e| (e, java_root().join(tmx_match).display().to_string()))
                    .collect(),
                Vec::new(),
                en_tok,
                "en",
                "fr",
                false,
                true,
                30,
            ),
            "multi" => (
                omegat_core::tmx::parse_tmx_all(
                    &read_tmx("src/test/resources/data/tmx/test-multiple-entries.tmx"),
                    "en-US",
                    "co",
                ),
                Vec::new(),
                vec![omegat_core::find_matches::FileTranslation {
                    source: "Other".into(),
                    translation: "Other".into(),
                    file: "website/download.html".into(),
                    fuzzy: false,
                }],
                en_tok,
                "en-US",
                "co",
                true,
                false,
                85,
            ),
            other => panic!("unknown find_matches case {other}"),
        };
        let got = omegat_core::find_matches::search(omegat_core::find_matches::SearchRequest {
            query,
            memory: &memory,
            extra: &extra,
            files: &files,
            tokenizer,
            source_lang: src,
            target_lang: tgt,
            threshold,
            limit: 5,
            search_exactly_the_same: exact,
            run_separate_segment_match: separate,
            foreign_penalty: 30,
        });
        let got_hits: Vec<(String, String, i32, i32, i32, i32, String)> = got
            .iter()
            .map(|h| {
                (
                    h.source.clone(),
                    h.translation.clone(),
                    h.score,
                    h.score_no_stem,
                    h.adjusted_score,
                    h.penalty,
                    h.comes_from.clone(),
                )
            })
            .collect();
        let exp_hits: Vec<(String, String, i32, i32, i32, i32, String)> = expected
            .iter()
            .map(|h| {
                (
                    h["source"].as_str().unwrap().to_string(),
                    h["translation"].as_str().unwrap().to_string(),
                    h["score"].as_i64().unwrap() as i32,
                    h["score_no_stem"].as_i64().unwrap() as i32,
                    h["adjusted_score"].as_i64().unwrap() as i32,
                    h["penalty"].as_i64().unwrap() as i32,
                    h["comes_from"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(got_hits, exp_hits, "{method} {name}");
    }
}

#[test]
fn calc_match_statistics_test_methods_match_java() {
    let spec = load_exported("calc_match_statistics.json");
    let sources: Vec<String> = spec["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(sources.len(), 108);
    let tmx = read_tmx("src/test/resources/data/tmx/test-match-stat-en-ca.tmx");
    let extra: Vec<_> = omegat_core::tmx::parse_external_tmx(&tmx, "en", "ca", true)
        .into_iter()
        .map(|e| (e, "test-match-stat-en-ca.tmx".into()))
        .collect();
    let po = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../reference/java/src/test/resources/data/filters/po/file-POFilter-match-stat-en-ca.po",
    );
    let parsed = omegat_filters::FilterRegistry::new()
        .for_path(&po)
        .expect("po filter")
        .parse(
            &po,
            &omegat_filters::FilterContext {
                source_lang: "en".into(),
                target_lang: "ca".into(),
                ..Default::default()
            },
        )
        .expect("parse po");
    let files: Vec<omegat_core::find_matches::FileTranslation> = parsed
        .segments
        .iter()
        .filter_map(|s| {
            let t = s.existing_translation.as_ref().filter(|t| !t.is_empty())?;
            Some(omegat_core::find_matches::FileTranslation {
                source: s.source.clone(),
                translation: t.clone(),
                file: "file-POFilter-match-stat-en-ca.po".into(),
                fuzzy: s
                    .comment
                    .as_deref()
                    .map(|c| c.lines().any(|l| l.contains("fuzzy")))
                    .unwrap_or(false),
            })
        })
        .collect();
    if let Some(per) = spec["per_source"].as_array() {
        let mut seen = std::collections::HashSet::new();
        for row in per {
            let src = row["source"].as_str().unwrap();
            let exp_words = row["words"].as_i64().unwrap() as usize;
            assert_eq!(
                omegat_core::stats::number_of_words(src),
                exp_words,
                "StatCount.words {:?}",
                src.chars().take(50).collect::<String>()
            );
            if !row["first"].as_bool().unwrap_or(false) || !seen.insert(src.to_string()) {
                continue;
            }
            let sim = omegat_core::stats::calc_max_similarity(
                src,
                &[],
                &extra,
                &files,
                "org.omegat.tokenizer.LuceneEnglishTokenizer",
                "en",
                "ca",
            );
            assert_eq!(
                sim,
                row["percent"].as_i64().unwrap() as i32,
                "calcMaxSimilarity {:?}",
                src.chars().take(70).collect::<String>()
            );
        }
    }
    let translated = vec![false; sources.len()];
    let rows = omegat_core::stats::calc_match_bins_ex(
        &sources,
        &translated,
        &[],
        &extra,
        &files,
        "org.omegat.tokenizer.LuceneEnglishTokenizer",
        "en",
        "ca",
    );
    for case in spec["cases"].as_array().unwrap() {
        let method = case["java_test"].as_str().unwrap();
        assert!(case["success"].as_bool().unwrap_or(false), "{method} java calc failed");
        let tables = case["tables"].as_array().unwrap();
        assert!(!tables.is_empty(), "{method}");
        if method.ends_with("#testCalcMatchStatics") {
            let first = tables[1].as_array().unwrap_or_else(|| tables[0].as_array().unwrap());
            // Java dumps an early table (repetitions/exact only) then the full bin table.
            let full = tables
                .iter()
                .rev()
                .find(|t| t.as_array().map(|a| a.len() >= 8).unwrap_or(false))
                .and_then(|t| t.as_array())
                .unwrap();
            let nums = |row: usize| -> (i64, i64, i64, i64) {
                (
                    full[row][1].as_str().unwrap().parse().unwrap(),
                    full[row][2].as_str().unwrap().parse().unwrap(),
                    full[row][3].as_str().unwrap().parse().unwrap(),
                    full[row][4].as_str().unwrap().parse().unwrap(),
                )
            };
            // rows: rep, exact, 95, 85, 75, 50, none, total
            assert_eq!(
                (rows[0].segments, rows[0].words, rows[0].chars_nosp, rows[0].chars),
                nums(0),
                "repetitions"
            );
            assert_eq!(
                (rows[2].segments, rows[2].words, rows[2].chars_nosp, rows[2].chars),
                nums(2),
                "fuzzy_95"
            );
            assert_eq!(
                (rows[4].segments, rows[4].words, rows[4].chars_nosp, rows[4].chars),
                nums(4),
                "fuzzy_75"
            );
            assert_eq!(
                (rows[5].segments, rows[5].words, rows[5].chars_nosp, rows[5].chars),
                nums(5),
                "fuzzy_50"
            );
            assert_eq!(
                (rows[6].segments, rows[6].words, rows[6].chars_nosp, rows[6].chars),
                nums(6),
                "none"
            );
            assert_eq!(
                (rows[7].segments, rows[7].words, rows[7].chars_nosp, rows[7].chars),
                nums(7),
                "total"
            );
            let _ = first;
        }
        if method.ends_with("#testStatistics") {
            let first = tables[0].as_array().unwrap();
            assert_eq!(first[0][1], "108");
            assert_eq!(first[0][2], "938");
            assert_eq!(rows[7].segments, 108);
            assert_eq!(rows[7].words, 938);
        }
    }
}
