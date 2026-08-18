//! assert_eq against ExportGoldens rewrite-wave JSON (one file per java_test).

use serde_json::Value;
use std::path::PathBuf;

fn golden(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/goldens").join(rel);
    assert!(path.is_file(), "missing {}", path.display());
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["exported_by"].as_str(), Some("org.omegat.tools.ExportGoldens"));
    v
}

#[test]
fn string_util_title_and_spaces_match_java() {
    let g = golden("util/StringUtilTest#testIsTitleCase.json");
    for c in g["cases"].as_array().unwrap() {
        let input = c["input"].as_str().unwrap();
        assert_eq!(
            omegat_core::string_util::is_title_case(input),
            c["title"].as_bool().unwrap(),
            "{input}"
        );
    }
    let ws = golden("util/StringUtilTest#testIsWhiteSpace.json");
    assert_eq!(omegat_core::string_util::is_white_space(""), ws["empty"].as_bool().unwrap());
    assert_eq!(omegat_core::string_util::is_white_space(" "), ws["space"].as_bool().unwrap());
    assert_eq!(omegat_core::string_util::is_white_space(" a "), ws["mixed"].as_bool().unwrap());
    assert_eq!(
        omegat_core::string_util::is_white_space("\u{00a0}\u{2007}\u{202f}"),
        ws["nbsp"].as_bool().unwrap()
    );
}

#[test]
fn language_and_bidi_match_java() {
    let g = golden("util/LanguageTest#testGetLocale.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("XXX-yy")).get_locale_code(),
        g["XXX-yy"].as_str().unwrap()
    );
    let space = golden("util/LanguageTest#testIsSpaceDelimited.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("en")).is_space_delimited(),
        space["en"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::language::Language::new(Some("zh")).is_space_delimited(),
        space["zh"].as_bool().unwrap()
    );
    let bidi = golden("util/BiDiUtilsTest#testGetOrientationType_noProjectLocaleRtl_allRtl.json");
    assert_eq!(omegat_core::bidi::is_rtl("ar"), bidi["rtl"].as_bool().unwrap());
}

#[test]
fn file_util_and_searcher_match_java() {
    let rel = golden("util/FileUtilTest#testRelative.json");
    assert_eq!(omegat_core::file_util::is_relative("C:\\zz"), rel["win"].as_bool().unwrap());
    assert_eq!(omegat_core::file_util::is_relative("/zz"), rel["unix"].as_bool().unwrap());
    assert_eq!(omegat_core::file_util::is_relative("zz/"), rel["rel"].as_bool().unwrap());
    let mask = golden("util/FileUtilTest#testCompileFileMask.json");
    assert_eq!(
        omegat_core::file_util::compile_file_mask("Ab1-&*/**"),
        mask["pattern"].as_str().unwrap()
    );
    let names = golden("util/FileUtilTest#testGetUniqueNames.json");
    let got = omegat_core::file_util::get_unique_names(&[
        "/foo/foo.txt".into(),
        "/foo/bar.txt".into(),
        "/bar/bar.txt".into(),
    ]);
    let want: Vec<String> = names["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, want);

    let exact = golden("search/SearcherTest#testSearchStringExactMatch.json");
    let mut expr = omegat_core::search::SearchExpression::exact("OmegaT is great", true);
    for c in exact["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::search::search_string(c["text"].as_str().unwrap(), &expr),
            c["hit"].as_bool().unwrap()
        );
    }
    let whole = golden("search/SearcherTest#testSearchStringUnicodeWholeWordsOnly.json");
    expr = omegat_core::search::SearchExpression::exact("слово", false);
    expr.whole_words = true;
    for c in whole["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::search::search_string(c["text"].as_str().unwrap(), &expr),
            c["hit"].as_bool().unwrap(),
            "{}",
            c["text"]
        );
    }
}

#[test]
fn tmx_reader_level1_matches_java() {
    let g = golden("engine/TMXReaderTest#testLeveL1.json");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/java/src/test/resources/data/tmx/test-level1.tmx");
    let tmx = omegat_core::tmx::ProjectTmx::load(&path, "en-US", "be").unwrap();
    let pairs = g["pairs"].as_object().unwrap();
    assert_eq!(tmx.entries.len(), g["count"].as_u64().unwrap() as usize);
    for (src, tgt) in pairs {
        assert_eq!(
            tmx.get(src).map(|e| e.translation.as_str()),
            Some(tgt.as_str().unwrap()),
            "{src}"
        );
    }
}

#[test]
fn searcher_replace_regex_matches_java() {
    let g = golden("search/SearcherTest#testSearchReplaceRegexMatch.json");
    let mut expr = omegat_core::search::SearchExpression::exact(g["query"].as_str().unwrap(), false);
    expr.kind = omegat_core::search::SearchKind::Regex;
    expr.replacement = Some(g["replacement"].as_str().unwrap().into());
    let hits = omegat_core::search::search_replace_matches(g["input"].as_str().unwrap(), &expr);
    let got: Vec<String> = hits.into_iter().map(|h| h.replacement).collect();
    let want: Vec<String> = g["replacements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, want);
}

#[test]
fn project_properties_team_and_export_levels() {
    let git = golden("engine/ProjectPropertiesTest#testIsTeamProjectOnGitTeam.json");
    let mut props = omegat_core::properties::ProjectProperties::create(
        std::path::PathBuf::from("/tmp"),
        "en".into(),
        "fr".into(),
        false,
    );
    props.repositories.push(omegat_core::properties::RepositoryDef {
        repo_type: "git".into(),
        url: "https://example.com/example.git".into(),
        branch: Some("main".into()),
        mappings: vec![omegat_core::properties::RepositoryMapping {
            local: String::new(),
            repository: String::new(),
            includes: vec![],
            excludes: vec![],
        }],
    });
    assert_eq!(props.is_team_project(), git["team"].as_bool().unwrap());
    let all = golden("engine/ProjectPropertiesTest#testSetExportTMLevelsAll.json");
    props.set_export_tm_levels(true, true, true);
    let want: Vec<String> = all["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(props.export_tm_level_list(), want);
}

#[test]
fn string_util_full_java_methods() {
    let after = golden("util/StringUtilTest#testIsSubstringAfter.json");
    for c in after["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::string_util::is_substring_after(
                c["text"].as_str().unwrap(),
                c["pos"].as_u64().unwrap() as usize,
                c["sub"].as_str().unwrap(),
            ),
            c["after"].as_bool().unwrap()
        );
    }
    let before = golden("util/StringUtilTest#testIsSubstringBefore.json");
    for c in before["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::string_util::is_substring_before(
                c["text"].as_str().unwrap(),
                c["pos"].as_u64().unwrap() as usize,
                c["sub"].as_str().unwrap(),
            ),
            c["before"].as_bool().unwrap()
        );
    }
    let xml = golden("util/StringUtilTest#testIsValidXMLChar.json");
    assert_eq!(omegat_core::string_util::is_valid_xml_char(0x01), xml["c01"].as_bool().unwrap());
    assert_eq!(omegat_core::string_util::is_valid_xml_char(0x09), xml["c09"].as_bool().unwrap());
    let list = golden("util/StringUtilTest#testConvertToList.json");
    let got = omegat_core::string_util::convert_to_list("  omegat   level1  level2  ");
    let want: Vec<String> = list["list"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, want);
    let wrap = golden("util/StringUtilTest#testWrapBasicFunctionality.json");
    assert_eq!(
        omegat_core::string_util::wrap("This is a test", 7),
        wrap["a"].as_str().unwrap()
    );
    let strip = golden("util/StringUtilTest#testStripFromEnd.json");
    assert_eq!(
        omegat_core::string_util::strip_from_end("file.txt.bak", &[".bak"]),
        strip["a"].as_str().unwrap()
    );
}

#[test]
fn searcher_check_entry_and_project_hits() {
    let src = golden("search/SearcherTest#testSearchCheckEntrySrcText.json");
    let mut expr = omegat_core::search::SearchExpression::exact("OmegaT is great", true);
    let hits = omegat_core::search::check_entry("OmegaT is great", None, None, None, None, &expr);
    assert_eq!(!hits.is_empty(), src["hit"].as_bool().unwrap());
    let loc = golden("search/SearcherTest#testSearchCheckEntryLocalizedText.json");
    let hits = omegat_core::search::check_entry("", Some("OmegaT is great"), None, None, None, &expr);
    assert_eq!(!hits.is_empty(), loc["hit"].as_bool().unwrap());
    let note = golden("search/SearcherTest#testSearchCheckEntryNote.json");
    let hits = omegat_core::search::check_entry("", None, Some("OmegaT is great"), None, None, &expr);
    assert_eq!(!hits.is_empty(), note["hit"].as_bool().unwrap());
    let comments = golden("search/SearcherTest#testSearchCheckEntryComments.json");
    expr = omegat_core::search::SearchExpression::exact("Comment 2", true);
    let hits = omegat_core::search::check_entry("", None, None, Some(&["Comment 1", "Comment 2"]), None, &expr);
    assert_eq!(!hits.is_empty(), comments["hit"].as_bool().unwrap());
    let author = golden("search/SearcherTest#testSearchCheckEntryAuthor.json");
    expr = omegat_core::search::SearchExpression::exact("OmegaT is great", true);
    expr.search_author = true;
    expr.author = Some("author 1".into());
    let hits = omegat_core::search::check_entry(
        "OmegaT is great",
        None,
        None,
        None,
        Some("author 1"),
        &expr,
    );
    assert_eq!(!hits.is_empty(), author["hit"].as_bool().unwrap());
    let not_author = golden("search/SearcherTest#testSearchCheckEntryNotAuthor.json");
    let hits = omegat_core::search::check_entry(
        "OmegaT is great",
        None,
        None,
        None,
        Some("author 2"),
        &expr,
    );
    assert_eq!(!hits.is_empty(), not_author["hit"].as_bool().unwrap());
    let empty = golden("search/SearcherTest#testGetSearchResultsEmpty.json");
    assert_eq!(empty["count"].as_u64().unwrap(), 0);
    let multi = golden("search/SearcherTest#testSearchStringMultipleMatches.json");
    expr = omegat_core::search::SearchExpression::exact("OmegaT", false);
    expr.kind = omegat_core::search::SearchKind::Keyword;
    assert_eq!(
        omegat_core::search::search_string("OmegaT is great, OmegaT helps you translate", &expr),
        multi["hit"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::search::search_replace_matches("OmegaT is great, OmegaT helps you translate", &expr)
            .len() as u64,
        multi["count"].as_u64().unwrap()
    );
}

#[test]
fn real_project_import_matches_java() {
    let same = golden("engine/RealProjectTest#testImportSameTranslations.json");
    let src = "List of sections in %s";
    let tr1 = "Liste des sections de %s";
    let tr2 = "Ceci est la liste des sections de %s";
    let entries = vec![
        omegat_core::import::SourceImport {
            id: "id1".into(),
            source: src.into(),
            source_translation: Some(tr1.into()),
            fuzzy: false,
        },
        omegat_core::import::SourceImport {
            id: "id2".into(),
            source: src.into(),
            source_translation: Some(tr1.into()),
            fuzzy: false,
        },
        omegat_core::import::SourceImport {
            id: "id3".into(),
            source: src.into(),
            source_translation: Some(tr2.into()),
            fuzzy: false,
        },
    ];
    let mut tmx = omegat_core::tmx::ProjectTmx::new();
    omegat_core::import::import_translations_from_sources(&mut tmx, &entries, true, true);
    assert_eq!(
        tmx.get_default_translation(src).map(|e| e.translation.as_str()),
        Some(same["default"].as_str().unwrap())
    );
    assert!(tmx.get_multiple_translation("id1", src).is_none());
    assert!(tmx.get_multiple_translation("id2", src).is_none());
    assert_eq!(
        tmx.get_multiple_translation("id3", src).map(|e| e.translation.as_str()),
        Some(same["alt_id3"].as_str().unwrap())
    );

    let fuzzy = golden("engine/RealProjectTest#testImportFuzzy.json");
    let mut tmx = omegat_core::tmx::ProjectTmx::new();
    omegat_core::import::import_translations_from_sources(
        &mut tmx,
        &[omegat_core::import::SourceImport {
            id: "id1".into(),
            source: src.into(),
            source_translation: Some(tr1.into()),
            fuzzy: true,
        }],
        true,
        true,
    );
    assert_eq!(tmx.get_default_translation(src).is_some(), fuzzy["has_default"].as_bool().unwrap());

    let over = golden("engine/RealProjectTest#testImportOverwrite.json");
    let mut tmx = omegat_core::tmx::ProjectTmx::new();
    tmx.set_default_translation(src, "exist");
    omegat_core::import::import_translations_from_sources(
        &mut tmx,
        &[
            omegat_core::import::SourceImport {
                id: "id1".into(),
                source: src.into(),
                source_translation: Some(tr1.into()),
                fuzzy: false,
            },
            omegat_core::import::SourceImport {
                id: "id2".into(),
                source: src.into(),
                source_translation: Some(tr2.into()),
                fuzzy: false,
            },
        ],
        true,
        true,
    );
    assert_eq!(
        tmx.get_default_translation(src).map(|e| e.translation.as_str()),
        Some(over["default"].as_str().unwrap())
    );
}

#[test]
fn srx_manager_default_matches_java() {
    let g = golden("engine/SRXManagerTest#testGetDefaultVersion.json");
    let srx = omegat_core::srx::SrxManager::get_default();
    assert_eq!(srx.version, g["version"].as_str().unwrap());
    let tags = golden("engine/SRXManagerTest#testGetDefaultIncludeEndingTagsIsTrue.json");
    assert_eq!(srx.include_ending_tags, tags["include_ending_tags"].as_bool().unwrap());
    let sub = golden("engine/SRXManagerTest#testGetDefaultSegmentSubflowsIsTrue.json");
    assert_eq!(srx.segment_subflows, sub["segment_subflows"].as_bool().unwrap());
    let n = golden("engine/SRXManagerTest#testGetDefaultMappingRulesHas18.json");
    assert_eq!(srx.mapping_rules as u64, n["count"].as_u64().unwrap());
    let cve = golden("engine/SRXTest#testSRXLoaderSecureCVE_2024_51366.json");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("segmentation.conf"),
        "<java><object class=\"java.lang.ProcessBuilder\"></object></java>",
    )
    .unwrap();
    let loaded = omegat_core::srx::SrxManager::load_from_dir(dir.path());
    assert_eq!(loaded.is_some(), cve["loaded"].as_bool().unwrap());
    assert!(!dir.path().join("test-file").exists());
}

#[test]
fn tokenizer_verbatim_and_contains_match_java() {
    let g = golden("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithMultipleWords.json");
    let tokens = omegat_core::tokenize::tokenize_verbatim(g["input"].as_str().unwrap());
    let want: Vec<String> = g["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(tokens, want);
    let empty = golden("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithEmptyString.json");
    assert_eq!(
        omegat_core::tokenize::tokenize_verbatim("").len() as u64,
        empty["count"].as_u64().unwrap()
    );
    let ws = golden("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithWhitespace.json");
    assert_eq!(
        omegat_core::tokenize::tokenize_verbatim("     ").len() as u64,
        ws["count"].as_u64().unwrap()
    );
    let mix = golden("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithMixedAlphanumeric.json");
    let tokens = omegat_core::tokenize::tokenize_verbatim(mix["input"].as_str().unwrap());
    let want: Vec<String> = mix["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(tokens, want);
    let contains = golden("tokenize/DefaultTokenizerTest#testContains.json");
    let text = contains["text"].as_str().unwrap();
    let tokens = omegat_core::tokenize::tokenize_verbatim(text);
    assert!(omegat_core::tokenize::is_contains(&tokens, "quick"));
    assert!(!omegat_core::tokenize::is_contains(&tokens, "elephant"));
    let all = golden("tokenize/DefaultTokenizerTest#testContainsAll.json");
    let tokens = omegat_core::tokenize::tokenize_verbatim(all["text"].as_str().unwrap());
    let brown: Vec<String> = "The brown".split_whitespace().map(|s| s.to_string()).collect();
    assert_eq!(
        omegat_core::tokenize::is_contains_all(&tokens, &brown, true),
        all["the_brown_inexact"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::tokenize::is_contains_all(&tokens, &brown, false),
        all["the_brown_exact"].as_bool().unwrap()
    );
}

#[test]
fn glossary_searcher_english_and_cjk() {
    let en = golden("glossary/GlossarySearcherTest#testGlossarySearcherEnglish.json");
    let searcher = omegat_core::glossary::GlossarySearcher::new("en", "de", "org.omegat.tokenizer.LuceneEnglishTokenizer");
    let entries = [omegat_core::glossary::GlossaryEntry::new("source", "translation", "comment")];
    let hits = searcher.search_source_matches("source", &entries);
    assert_eq!(hits.len() as u64, en["count"].as_u64().unwrap());
    assert_eq!(hits[0].source, en["source"].as_str().unwrap());
    assert_eq!(hits[0].target, en["target"].as_str().unwrap());
    let cjk = golden("glossary/GlossarySearcherTest#testIsCjkMatchJapanese.json");
    assert_eq!(
        omegat_core::glossary::is_cjk_match("場所", "場所"),
        cjk["same"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::glossary::is_cjk_match("場所", "塗布"),
        cjk["other"].as_bool().unwrap()
    );
    let empty = golden("glossary/GlossarySearcherTest#testSearchSourceMatchesEmptyEntries.json");
    assert_eq!(
        searcher.search_source_matches("source", &[]).len() as u64,
        empty["count"].as_u64().unwrap()
    );
}

#[test]
fn issues_table_model_matches_java() {
    let rows = golden("gui/IssuesTableModelTest-testGetRowCount.json");
    let model = omegat_core::issues::IssuesTableModel::new(vec![omegat_core::issues::Issue {
        entry_num: 1,
        type_name: "Tag".into(),
        description: "MISSING".into(),
    }]);
    assert_eq!(model.row_count() as u64, rows["row_count"].as_u64().unwrap());
    let cols = golden("gui/IssuesTableModelTest-testGetColumnCount.json");
    assert_eq!(model.column_count() as u64, cols["column_count"].as_u64().unwrap());
    let term = golden("gui/TerminologyIssueProviderTest-testNonEmptyTargetTermReturnsTrue.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&["house"]),
        term["has_target"].as_bool().unwrap()
    );
    let empty_term = golden("gui/TerminologyIssueProviderTest-testEmptyTargetTermReturnsFalse.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&[""]),
        empty_term["has_target"].as_bool().unwrap()
    );
}

