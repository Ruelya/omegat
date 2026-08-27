//! assert_eq against ExportGoldens rewrite-wave JSON (one file per java_test).

use serde_json::Value;
use std::path::PathBuf;

fn golden(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens")
        .join(rel);
    assert!(path.is_file(), "missing {}", path.display());
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        v["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens")
    );
    v
}

fn searcher_golden(method: &str) -> Value {
    let value = golden(&format!("search/SearcherTest#{method}.json"));
    let expected = format!("org.omegat.core.search.SearcherTest#{method}");
    assert_eq!(
        value["java_test"].as_str(),
        Some(expected.as_str())
    );
    value
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
    assert_eq!(
        omegat_core::string_util::is_white_space(""),
        ws["empty"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_white_space(" "),
        ws["space"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_white_space(" a "),
        ws["mixed"].as_bool().unwrap()
    );
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
    assert_eq!(
        omegat_core::bidi::is_rtl("ar"),
        bidi["rtl"].as_bool().unwrap()
    );
}

#[test]
fn file_util_and_searcher_match_java() {
    let rel = golden("util/FileUtilTest#testRelative.json");
    assert_eq!(
        omegat_core::file_util::is_relative("C:\\zz"),
        rel["win"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::file_util::is_relative("/zz"),
        rel["unix"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::file_util::is_relative("zz/"),
        rel["rel"].as_bool().unwrap()
    );
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
    let mut expr =
        omegat_core::search::SearchExpression::exact(g["query"].as_str().unwrap(), false);
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
    props
        .repositories
        .push(omegat_core::properties::RepositoryDef {
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
    assert_eq!(
        omegat_core::string_util::is_valid_xml_char(0x01),
        xml["c01"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_valid_xml_char(0x09),
        xml["c09"].as_bool().unwrap()
    );
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
    let hits =
        omegat_core::search::check_entry("", Some("OmegaT is great"), None, None, None, &expr);
    assert_eq!(!hits.is_empty(), loc["hit"].as_bool().unwrap());
    let note = golden("search/SearcherTest#testSearchCheckEntryNote.json");
    let hits =
        omegat_core::search::check_entry("", None, Some("OmegaT is great"), None, None, &expr);
    assert_eq!(!hits.is_empty(), note["hit"].as_bool().unwrap());
    let comments = golden("search/SearcherTest#testSearchCheckEntryComments.json");
    expr = omegat_core::search::SearchExpression::exact("Comment 2", true);
    let hits = omegat_core::search::check_entry(
        "",
        None,
        None,
        Some(&["Comment 1", "Comment 2"]),
        None,
        &expr,
    );
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
        omegat_core::search::search_replace_matches(
            "OmegaT is great, OmegaT helps you translate",
            &expr
        )
        .len() as u64,
        multi["count"].as_u64().unwrap()
    );
}

#[test]
fn searcher_all_java_methods_use_stateful_product_path() {
    use omegat_core::search::{
        ProjectSearchEntry, SearchExpression, SearchKind, SearchMode, SearchOrigin, Searcher,
    };

    let string_cases = [
        (
            "testSearchStringExactMatch",
            SearchKind::Exact,
            true,
            false,
            false,
        ),
        (
            "testSearchStringKeywordMatch",
            SearchKind::Keyword,
            false,
            false,
            false,
        ),
        (
            "testSearchStringExactWholeWordsOnly",
            SearchKind::Exact,
            false,
            true,
            false,
        ),
        (
            "testSearchStringKeywordWholeWordsOnly",
            SearchKind::Keyword,
            false,
            true,
            false,
        ),
        (
            "testSearchStringWildcardWholeWordsOnly",
            SearchKind::Exact,
            false,
            true,
            false,
        ),
        (
            "testSearchStringUnicodeWholeWordsOnly",
            SearchKind::Exact,
            false,
            true,
            false,
        ),
        (
            "testSearchStringWholeWordsOnlyIgnoredForRegex",
            SearchKind::Regex,
            false,
            true,
            false,
        ),
        (
            "testSearchStringRegexMatch",
            SearchKind::Regex,
            false,
            false,
            false,
        ),
        (
            "testSearchStringWidthInsensitive",
            SearchKind::Exact,
            false,
            false,
            true,
        ),
        (
            "testSearchStringEmptyInput",
            SearchKind::Exact,
            true,
            false,
            false,
        ),
        (
            "testSearchStringNoMatch",
            SearchKind::Exact,
            false,
            false,
            false,
        ),
    ];
    for (method, kind, case_sensitive, whole_words, width_insensitive) in string_cases {
        let g = searcher_golden(method);
        let mut expression = SearchExpression::exact(g["query"].as_str().unwrap(), case_sensitive);
        expression.kind = kind;
        expression.whole_words = whole_words;
        expression.width_insensitive = width_insensitive;
        let mut searcher = Searcher::new(expression);
        for case in g["cases"].as_array().unwrap() {
            assert_eq!(
                searcher.search_string(case["text"].as_str(), true),
                case["hit"].as_bool().unwrap(),
                "{method}: {}",
                case["text"]
            );
        }
    }

    let partial = searcher_golden("testSearchStringPartialRegexMatch");
    let mut searcher = Searcher::new(SearchExpression::regex(r"version \d+\.\d+\.\d+", false));
    assert_eq!(
        searcher.search_string(partial["text"].as_str(), true),
        partial["hit"].as_bool().unwrap()
    );
    let null = searcher_golden("testSearchStringNullInput");
    assert_eq!(
        searcher.search_string(None, true),
        null["hit"].as_bool().unwrap()
    );

    for (method, kind) in [
        ("testSearchReplaceExactMatch", SearchKind::Exact),
        ("testSearchReplaceRegexMatch", SearchKind::Regex),
        ("testSearchReplaceKeywordNotSupported", SearchKind::Keyword),
    ] {
        let g = searcher_golden(method);
        let mut expression = SearchExpression::exact(g["query"].as_str().unwrap(), false);
        expression.kind = kind;
        expression.mode = SearchMode::Replace;
        expression.replacement = Some(g["replacement"].as_str().unwrap().into());
        let mut searcher = Searcher::new(expression);
        searcher.run();
        assert!(
            searcher.search_string(g["input"].as_str(), false),
            "{method}"
        );
        let replacements: Vec<String> = searcher
            .get_found_matches()
            .unwrap()
            .iter()
            .map(|m| m.replacement.clone())
            .collect();
        let expected: Vec<String> = g["replacements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(replacements, expected, "{method}");
        assert_eq!(
            replacements.len() as u64,
            g["count"].as_u64().unwrap(),
            "{method}"
        );
    }

    for (method, expression) in [
        (
            "testGetExpressionExactMatch",
            SearchExpression::exact("OmegaT is great", true),
        ),
        (
            "testGetExpressionKeywordMatch",
            SearchExpression::keyword("great software", false),
        ),
        (
            "testGetExpressionRegexMatch",
            SearchExpression::regex(r"version \d+\.\d+\.\d+", false),
        ),
    ] {
        let g = searcher_golden(method);
        let original = expression.clone();
        let searcher = Searcher::new(expression);
        assert_eq!(
            searcher.get_expression() == &original,
            g["same"].as_bool().unwrap(),
            "{method}"
        );
    }

    let mut check = |method: &str,
                     query: &str,
                     source: &str,
                     translation: Option<&str>,
                     note: Option<&str>,
                     properties: Vec<(&str, &str)>,
                     creator: Option<&str>,
                     expected_field: &str| {
        let g = searcher_golden(method);
        let mut entry = ProjectSearchEntry::project(1, "source.txt", source, translation);
        entry.note = note.map(str::to_string);
        entry.properties = properties
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        entry.creator = creator.map(str::to_string);
        let mut expression = SearchExpression::exact(query, true);
        if creator.is_some() {
            expression.search_author = true;
            expression.author = Some("author 1".into());
        }
        let mut searcher = Searcher::with_entries(expression, vec![entry]);
        searcher.run();
        let results = searcher.get_search_results().unwrap();
        assert_eq!(!results.is_empty(), g["hit"].as_bool().unwrap(), "{method}");
        if let Some(result) = results.first() {
            let actual_field = if !result.source_matches.is_empty() {
                "source"
            } else if !result.target_matches.is_empty() {
                "translation"
            } else if !result.note_matches.is_empty() {
                "note"
            } else {
                "comments"
            };
            assert_eq!(actual_field, expected_field, "{method}");
        }
    };
    check(
        "testSearchCheckEntrySrcText",
        "OmegaT is great",
        "OmegaT is great",
        None,
        None,
        vec![],
        None,
        "source",
    );
    check(
        "testSearchCheckEntryLocalizedText",
        "OmegaT is great",
        "",
        Some("OmegaT is great"),
        None,
        vec![],
        None,
        "translation",
    );
    check(
        "testSearchCheckEntryNote",
        "OmegaT is great",
        "",
        None,
        Some("OmegaT is great"),
        vec![],
        None,
        "note",
    );
    check(
        "testSearchCheckEntryComments",
        "Comment 2",
        "",
        None,
        None,
        vec![("comment", "Comment 1"), ("comment", "Comment 2")],
        None,
        "comments",
    );
    check(
        "testSearchCheckEntryAuthor",
        "OmegaT is great",
        "OmegaT is great",
        None,
        None,
        vec![],
        Some("author 1"),
        "source",
    );
    check(
        "testSearchCheckEntryNotAuthor",
        "OmegaT is great",
        "OmegaT is great",
        None,
        None,
        vec![],
        Some("author 2"),
        "source",
    );

    let props = searcher_golden("testSearchProjectFindsKeyFields");
    for needle in props["needles"].as_array().unwrap() {
        let needle = needle.as_str().unwrap();
        let mut entry = ProjectSearchEntry::project(1, "chapter_one.html", "OmegaT is great", None);
        entry.id = Some("MSG_GREETING_42".into());
        entry.path = Some("body/p[3]".into());
        let mut expression = SearchExpression::exact(needle, true);
        let mut searcher = Searcher::with_entries(expression.clone(), vec![entry.clone()]);
        searcher.run();
        assert_eq!(
            searcher.get_search_results().unwrap().len() as u64,
            props["with_props"].as_u64().unwrap(),
            "{needle} with properties"
        );
        expression.search_comments = false;
        let mut searcher = Searcher::with_entries(expression, vec![entry]);
        searcher.run();
        assert_eq!(
            searcher.get_search_results().unwrap().len() as u64,
            props["without_props"].as_u64().unwrap(),
            "{needle} without properties"
        );
    }

    let multi = searcher_golden("testSearchStringMultipleMatches");
    let mut searcher = Searcher::new(SearchExpression::keyword("OmegaT", false));
    searcher.run();
    assert_eq!(
        searcher.search_string(Some("OmegaT is great, OmegaT helps you translate"), true),
        multi["hit"].as_bool().unwrap()
    );
    assert_eq!(
        searcher.get_found_matches().unwrap().len(),
        multi["count"].as_u64().unwrap() as usize
    );
    let collapse = searcher_golden("testSearchStringCollapseResults");
    assert_eq!(
        searcher.search_string(Some("OmegaT OmegaT OmegaT"), true),
        collapse["hit"].as_bool().unwrap()
    );
    assert_eq!(
        searcher.get_found_matches().unwrap().len(),
        collapse["count"].as_u64().unwrap() as usize
    );

    let basic = searcher_golden("testSearch");
    let entries = vec![ProjectSearchEntry::project(
        1,
        "source.txt",
        "List of sections in %s",
        Some("Liste des sections de %s"),
    )];
    let mut searcher = Searcher::with_entries(SearchExpression::keyword("list", false), entries);
    searcher.run();
    assert_eq!(
        searcher.get_search_results().unwrap().len() as u64,
        basic["count"].as_u64().unwrap()
    );

    let empty = searcher_golden("testGetSearchResultsEmpty");
    let searcher = Searcher::new(SearchExpression::exact("OmegaT is great", true));
    assert!(searcher.get_search_results().is_err());
    assert_eq!(empty["count"].as_u64().unwrap(), 0);

    for (method, expression, source, translation) in [
        (
            "testGetSearchResultsExactMatch",
            SearchExpression::exact("OmegaT is great", true),
            "OmegaT is great",
            "OmegaT est génial",
        ),
        (
            "testGetSearchResultsKeywordMatch",
            SearchExpression::keyword("great software", false),
            "OmegaT is great software",
            "OmegaT est un génial logiciel",
        ),
    ] {
        let g = searcher_golden(method);
        let entries = vec![
            ProjectSearchEntry::project(1, "source.txt", source, Some(translation)),
            ProjectSearchEntry::orphan(source, Some(translation)),
        ];
        let mut searcher = Searcher::with_entries(expression, entries);
        searcher.run();
        let results = searcher.get_search_results().unwrap();
        assert_eq!(
            results.len() as u64,
            g["count"].as_u64().unwrap(),
            "{method}"
        );
        assert_eq!(results[0].source, g["src"].as_str().unwrap(), "{method}");
    }

    let modified = searcher_golden("testGetSearchResultsAfterModification");
    let mut searcher = Searcher::with_entries(
        SearchExpression::exact("OmegaT is great", true),
        vec![
            ProjectSearchEntry::project(
                1,
                "source.txt",
                "OmegaT is great",
                Some("OmegaT est génial"),
            ),
            ProjectSearchEntry::orphan("OmegaT is great", Some("OmegaT est génial")),
        ],
    );
    searcher.run();
    assert_eq!(
        searcher.get_search_results().unwrap().len() as u64,
        modified["initial"].as_u64().unwrap()
    );
    searcher.entries_mut().push(ProjectSearchEntry::project(
        2,
        "source.txt",
        "OmegaT is fantastic",
        Some("OmegaT est fantastique"),
    ));
    searcher.run();
    assert_eq!(
        searcher.get_search_results().unwrap().len() as u64,
        modified["updated"].as_u64().unwrap()
    );

    let duplicates = searcher_golden("testGetSearchResultsHandlesDuplicates");
    let mut expression = SearchExpression::exact("Duplicate entry", true);
    expression.all_results = false;
    let entries = vec![
        ProjectSearchEntry::project(1, "source.txt", "Duplicate entry", Some("Entrée dupliquée")),
        ProjectSearchEntry::project(2, "source.txt", "Duplicate entry", Some("Entrée dupliquée")),
        ProjectSearchEntry {
            origin: SearchOrigin::Orphan {
                preamble: "Orphan segment".into(),
            },
            ..ProjectSearchEntry::project(0, "", "Duplicate entry", Some("Entrée dupliquée"))
        },
    ];
    let mut searcher = Searcher::with_entries(expression, entries);
    searcher.run();
    assert_eq!(
        searcher.get_search_results().unwrap().len() as u64,
        duplicates["count"].as_u64().unwrap()
    );
    assert_eq!(
        searcher.get_search_results().unwrap()[0]
            .preamble
            .as_deref(),
        Some("source.txt +1\u{00a0}more")
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
        tmx.get_default_translation(src)
            .map(|e| e.translation.as_str()),
        Some(same["default"].as_str().unwrap())
    );
    assert!(tmx.get_multiple_translation("id1", src).is_none());
    assert!(tmx.get_multiple_translation("id2", src).is_none());
    assert_eq!(
        tmx.get_multiple_translation("id3", src)
            .map(|e| e.translation.as_str()),
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
    assert_eq!(
        tmx.get_default_translation(src).is_some(),
        fuzzy["has_default"].as_bool().unwrap()
    );

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
        tmx.get_default_translation(src)
            .map(|e| e.translation.as_str()),
        Some(over["default"].as_str().unwrap())
    );
}

#[test]
fn srx_manager_default_matches_java() {
    let g = golden("engine/SRXManagerTest#testGetDefaultVersion.json");
    let srx = omegat_core::srx::SrxManager::get_default();
    assert_eq!(srx.version, g["version"].as_str().unwrap());
    let tags = golden("engine/SRXManagerTest#testGetDefaultIncludeEndingTagsIsTrue.json");
    assert_eq!(
        srx.include_ending_tags,
        tags["include_ending_tags"].as_bool().unwrap()
    );
    let sub = golden("engine/SRXManagerTest#testGetDefaultSegmentSubflowsIsTrue.json");
    assert_eq!(
        srx.segment_subflows,
        sub["segment_subflows"].as_bool().unwrap()
    );
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
    let brown: Vec<String> = "The brown"
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
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
    let searcher = omegat_core::glossary::GlossarySearcher::new(
        "en",
        "de",
        "org.omegat.tokenizer.LuceneEnglishTokenizer",
    );
    let entries = [omegat_core::glossary::GlossaryEntry::new(
        "source",
        "translation",
        "comment",
    )];
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
    let model = omegat_core::issues::IssuesTableModel::new(vec![
        omegat_core::issues::Issue {
            entry_num: 1,
            type_name: "Test Issue 1".into(),
            description: "First test issue".into(),
        },
        omegat_core::issues::Issue {
            entry_num: 2,
            type_name: "Test Issue 2".into(),
            description: "Second test issue".into(),
        },
    ]);
    assert_eq!(
        model.row_count() as u64,
        rows["row_count"].as_u64().unwrap()
    );
    let cols = golden("gui/IssuesTableModelTest-testGetColumnCount.json");
    assert_eq!(
        model.column_count() as u64,
        cols["column_count"].as_u64().unwrap()
    );
    let term = golden("gui/TerminologyIssueProviderTest-testNonEmptyTargetTermReturnsTrue.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&["house"]),
        term["has_target"].as_bool().unwrap()
    );
    let empty_term =
        golden("gui/TerminologyIssueProviderTest-testEmptyTargetTermReturnsFalse.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&[""]),
        empty_term["has_target"].as_bool().unwrap()
    );
}

#[test]
fn string_util_language_bidi_file_util_remaining_match_java() {
    let alnum = golden("util/StringUtilTest#testAlphanumericStringCase.json");
    assert_eq!(
        omegat_core::string_util::is_upper_case("MQL5"),
        alnum["MQL5_upper"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_lower_case("mql5"),
        alnum["mql5_lower"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_title_case("Mql5"),
        alnum["Mql5_title"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_mixed_case("mQl5"),
        alnum["mQl5_mixed"].as_bool().unwrap()
    );

    let empty = golden("util/StringUtilTest#testEmptyStringCase.json");
    assert_eq!(
        omegat_core::string_util::is_upper_case(""),
        empty["empty_upper"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_lower_case(""),
        empty["empty_lower"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_title_case(""),
        empty["empty_title"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::to_title_case("", "en"),
        empty["empty_toTitle"].as_str().unwrap()
    );

    let mixed = golden("util/StringUtilTest#testIsMixedCase.json");
    assert_eq!(
        omegat_core::string_util::is_mixed_case("ABc"),
        mixed["ABc"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_mixed_case("Abc"),
        mixed["Abc"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_mixed_case(" {ABc"),
        mixed["braced"].as_bool().unwrap()
    );

    let nonword = golden("util/StringUtilTest#testNonWordCase.json");
    assert_eq!(
        omegat_core::string_util::is_lower_case("{"),
        nonword["lower"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_upper_case("{"),
        nonword["upper"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_title_case("{"),
        nonword["title"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_mixed_case("{"),
        nonword["mixed"].as_bool().unwrap()
    );

    let title = golden("util/StringUtilTest#testToTitleCase.json");
    assert_eq!(
        omegat_core::string_util::to_title_case("abc", "en"),
        title["abc"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::to_title_case("ijk", "tr"),
        title["tr"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::to_title_case("\u{01CC}", "en"),
        title["nj"].as_str().unwrap()
    );

    let cap = golden("util/StringUtilTest#testCapitalizeFirst.json");
    assert_eq!(
        omegat_core::string_util::capitalize_first("abc", "en"),
        cap["abc"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::capitalize_first("abC", "en"),
        cap["abC"].as_str().unwrap()
    );

    let mc = golden("util/StringUtilTest#testMatchCapitalization.json");
    assert_eq!(
        omegat_core::string_util::match_capitalization("foo", Some("Abc"), "en"),
        mc["title"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::match_capitalization("FOO", Some("lower"), "en"),
        mc["lower"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::match_capitalization("foo", Some("UPPER"), "en"),
        mc["upper"].as_str().unwrap()
    );

    let compress = golden("util/StringUtilTest#testCompressSpace.json");
    assert_eq!(
        omegat_core::string_util::compress_spaces(" One Two\nThree   Four\r\nFive "),
        compress["a"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::compress_spaces("Six\tseven"),
        compress["b"].as_str().unwrap()
    );

    let first = golden("util/StringUtilTest#testFirstN.json");
    let bmp = "𝐀𝐀";
    assert_eq!(
        omegat_core::string_util::first_n(bmp, 0),
        first["n0"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::first_n(bmp, 1),
        first["n1"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::first_n(bmp, 2),
        first["n2"].as_str().unwrap()
    );

    let trunc = golden("util/StringUtilTest#testTruncateString.json");
    let bmp3 = "𝐀𝐀𝐀";
    assert_eq!(
        omegat_core::string_util::truncate(bmp3, 1),
        trunc["n1"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::truncate(bmp3, 2),
        trunc["n2"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::truncate(bmp3, 3),
        trunc["n3"].as_str().unwrap()
    );

    let width = golden("util/StringUtilTest#testNormalizeWidth.json");
    assert_eq!(
        omegat_core::string_util::normalize_width(
            "\u{FF26}\u{FF4F}\u{FF4F}\u{3000}\u{FF11}\u{FF12}\u{FF13}"
        ),
        width["fw"].as_str().unwrap()
    );
    let conv = golden("util/StringUtilTest#testNormalizeWidthConversion.json");
    assert_eq!(
        omegat_core::string_util::normalize_width(
            "\u{FF21}\u{FF22}\u{FF23}\u{FF11}\u{FF12}\u{FF13}"
        ),
        conv["abc"].as_str().unwrap()
    );
    let punct = golden("util/StringUtilTest#testNormalizeWidthSpecialCharacters.json");
    assert_eq!(
        omegat_core::string_util::normalize_width(
            "\u{FF01}\u{FF1F}\u{FF08}\u{FF09}\u{FF5B}\u{FF5D}"
        ),
        punct["punct"].as_str().unwrap()
    );
    let spaces = golden("util/StringUtilTest#testNormalizeWidthSpaces.json");
    assert_eq!(
        omegat_core::string_util::normalize_width("a\u{00a0}b"),
        spaces["nbsp"].as_str().unwrap()
    );
    let edge = golden("util/StringUtilTest#testNormalizeWidthEdgeCases.json");
    assert_eq!(
        omegat_core::string_util::normalize_width(""),
        edge["empty"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::normalize_width("Already normalized"),
        edge["plain"].as_str().unwrap()
    );
    let hpa = golden("util/StringUtilTest#testReplaceSquaredLatinAbbreviations.json");
    assert_eq!(
        omegat_core::string_util::normalize_width("\u{3371}"),
        hpa["hpa"].as_str().unwrap()
    );
    let ka = golden("util/StringUtilTest#testProcessKatakana.json");
    assert_eq!(
        omegat_core::string_util::normalize_width("\u{FF76}"),
        ka["ka"].as_str().unwrap()
    );
    let hang = golden("util/StringUtilTest#testProcessHangul.json");
    assert_eq!(
        omegat_core::string_util::normalize_width("\u{FFBE}"),
        hang["h"].as_str().unwrap()
    );

    let rstrip = golden("util/StringUtilTest#testRstrip.json");
    assert_eq!(
        omegat_core::string_util::rstrip("abc  "),
        rstrip["a"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::rstrip("abc"),
        rstrip["b"].as_str().unwrap()
    );

    let wrap_e = golden("util/StringUtilTest#testWrapEdgeCases.json");
    assert_eq!(
        omegat_core::string_util::wrap("", 5),
        wrap_e["empty"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::wrap("Longword", 5),
        wrap_e["long"].as_str().unwrap()
    );

    let cmp = golden("util/StringUtilTest#testCompareToNullable.json");
    assert_eq!(
        omegat_core::string_util::compare_to_nullable(None, None),
        cmp["nn"].as_i64().unwrap() as i32
    );
    assert_eq!(
        omegat_core::string_util::compare_to_nullable(Some("a"), Some("a")),
        cmp["aa"].as_i64().unwrap() as i32
    );

    let rc = golden("util/StringUtilTest#testReplaceCaseBasicFunctionality.json");
    assert_eq!(
        omegat_core::string_util::replace_case("\\Uhello\\E", "en"),
        rc["u"].as_str().unwrap()
    );
    let rce = golden("util/StringUtilTest#testReplaceCaseEscapeSequences.json");
    assert_eq!(
        omegat_core::string_util::replace_case("\\\\", "en"),
        rce["q"].as_str().unwrap()
    );
    let rcedge = golden("util/StringUtilTest#testReplaceCaseEdgeCases.json");
    assert_eq!(
        omegat_core::string_util::replace_case("Hello, World!", "en"),
        rcedge["plain"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::replace_case("\\UHello", "en"),
        rcedge["U"].as_str().unwrap()
    );
    let casec = golden("util/StringUtilTest#testCaseConversion.json");
    assert_eq!(
        omegat_core::string_util::replace_case("\\uistanbul", "en"),
        casec["en"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::replace_case("\\uistanbul", "tr"),
        casec["tr"].as_str().unwrap()
    );

    let nonbmp = golden("util/StringUtilTest#testUnicodeNonBMP.json");
    assert_eq!(
        omegat_core::string_util::is_upper_case("𝐀"),
        nonbmp["upperA"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_title_case("𝐀"),
        nonbmp["titleA"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::string_util::is_title_case("𝐀𝐚"),
        nonbmp["titleAa"].as_bool().unwrap()
    );

    let lang = golden("util/LanguageTest#testGetLanguage.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("xx-YY")).get_language(),
        lang["xx-YY"].as_str().unwrap()
    );
    let ctor = golden("util/LanguageTest#testConstructor.json");
    assert_eq!(
        omegat_core::language::Language::new(None).get_language(),
        ctor["empty"].as_str().unwrap()
    );
    let eq = golden("util/LanguageTest#testEquals.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("xxx-YY"))
            == omegat_core::language::Language::new(Some("XXX-yy")),
        eq["eq"].as_bool().unwrap()
    );
    let bcp = golden("util/LanguageTest#testBCP47.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("en-KW-x-ukeng")).get_language_code(),
        bcp["code"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::language::Language::verify_single_lang_code("es-419"),
        bcp["es419"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::language::Language::verify_single_lang_code("xxx+ZZZ-a-BBB-ccc"),
        bcp["plus"].as_bool().unwrap()
    );
    let ar = golden(
        "util/LanguageTest#testGetLowerCaseLanguageFromLocale_languageAndCountryLocale.json",
    );
    assert_eq!(
        omegat_core::language::Language::new(Some("AR-DZ")).get_language_code(),
        ar["lang"].as_str().unwrap()
    );
    let es = golden("util/LanguageTest#testGetLowerCaseLanguageFromLocale_languageOnlyLocale.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("ES")).get_language_code(),
        es["lang"].as_str().unwrap()
    );
    let dz =
        golden("util/LanguageTest#testGetUpperCaseCountryFromLocale_languageAndCountryLocale.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("AR-DZ")).get_country_code(),
        dz["country"].as_str().unwrap()
    );
    let esc = golden("util/LanguageTest#testGetUpperCaseCountryFromLocale_languageOnlyLocale.json");
    assert_eq!(
        omegat_core::language::Language::new(Some("ES")).get_country_code(),
        esc["country"].as_str().unwrap()
    );

    let ltr = golden("util/BiDiUtilsTest#testAddLtrBidiAround.json");
    assert_eq!(
        omegat_core::bidi::add_ltr_bidi_around("x"),
        ltr["text"].as_str().unwrap()
    );
    let rtl = golden("util/BiDiUtilsTest#testAddRtlBidiAround.json");
    assert_eq!(
        omegat_core::bidi::add_rtl_bidi_around("x"),
        rtl["text"].as_str().unwrap()
    );
    let no_ltr = golden("util/BiDiUtilsTest#testGetOrientationType_noProjectLocaleLtr_allLtr.json");
    assert_eq!(
        omegat_core::bidi::is_rtl("pl"),
        no_ltr["rtl"].as_bool().unwrap()
    );
    let pair = golden("util/BiDiUtilsTest#testIsRtl_RtlLocale_true.json");
    assert_eq!(
        omegat_core::bidi::is_rtl("ar"),
        pair["rtl_ar"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::bidi::is_rtl("en"),
        pair["rtl_en"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::bidi::orientation_type(Some("en"), Some("fr"), "en"),
        omegat_core::bidi::Orientation::AllLtr
    );
    assert_eq!(
        omegat_core::bidi::orientation_type(Some("ar"), Some("he"), "ar"),
        omegat_core::bidi::Orientation::AllRtl
    );
    assert_eq!(
        omegat_core::bidi::orientation_type(Some("en"), Some("ar"), "en"),
        omegat_core::bidi::Orientation::Differ
    );

    let abs = golden("util/FileUtilTest#testAbsoluteForSystem.json");
    assert_eq!(
        omegat_core::file_util::absolute_for_system("C:\\zzz"),
        abs["converted"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::file_util::absolute_for_system("\\zzz"),
        abs["slash"].as_str().unwrap()
    );
    let eol = golden("util/FileUtilTest#testEOL.json");
    assert_eq!(
        omegat_core::file_util::get_eol(b"12\n34\n"),
        eol["lf"].as_str()
    );
    assert_eq!(
        omegat_core::file_util::get_eol(b"12\r34\r"),
        eol["cr"].as_str()
    );
    assert_eq!(
        omegat_core::file_util::get_eol(b"12\r\n34\r\n"),
        eol["crlf"].as_str()
    );
    let bak = golden("util/FileUtilTest#testBackupFilename.json");
    let got = omegat_core::file_util::get_backup_filename(
        std::path::Path::new("backup.test"),
        1684085727566,
    );
    assert_eq!(got, bak["pattern"].as_str().unwrap());
    let masks = golden("util/FileUtilTest#testFilePatterns.json");
    for c in masks["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::file_util::file_mask_matches(
                c["mask"].as_str().unwrap(),
                c["path"].as_str().unwrap()
            ),
            c["match"].as_bool().unwrap(),
            "{} {}",
            c["mask"],
            c["path"]
        );
    }
}
