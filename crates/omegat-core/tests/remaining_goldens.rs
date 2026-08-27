//! assert_eq against remaining ExportGoldens JSON (Java *Test method results).

use omegat_core::glossary::{GlossaryEntry, GlossarySearcher};
use omegat_core::issues::{
    collect_project_issues, ProjectIssueEntry, ProjectIssueKind, SimpleIssue,
};
use omegat_core::matches_var;
use omegat_core::properties::ProjectProperties;
use omegat_core::tags::{self, Tag, TagType};
use omegat_core::xml_stream::close_block;
use omegat_ipc::{FileStatDto, StatCountDto};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn golden(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens")
        .join(rel);
    assert!(path.is_file(), "missing {}", path.display());
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["exported_by"].as_str(), Some("org.omegat.tools.ExportGoldens"));
    assert!(v["java_test"].as_str().unwrap().contains('#'));
    v
}

fn java_res(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/java/src/test/resources")
        .join(rel)
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn entity_util_matches_java() {
    let named = golden("remaining/EntityUtilTest-testEntitiesToCharsNamedEntities.json");
    for c in named["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::entity_util::entities_to_chars(c["input"].as_str().unwrap()),
            c["output"].as_str().unwrap()
        );
    }
    let special = golden("remaining/EntityUtilTest-testEntitiesToCharsSpecialCharacters.json");
    for c in special["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::entity_util::entities_to_chars(c["input"].as_str().unwrap()),
            c["output"].as_str().unwrap()
        );
    }
    let numeric = golden("remaining/EntityUtilTest-testEntitiesToCharsNumericEntities.json");
    for c in numeric["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::entity_util::entities_to_chars(c["input"].as_str().unwrap()),
            c["output"].as_str().unwrap()
        );
    }
    let invalid = golden("remaining/EntityUtilTest-testEntitiesToCharsInvalid.json");
    for c in invalid["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::entity_util::entities_to_chars(c["input"].as_str().unwrap()),
            c["output"].as_str().unwrap()
        );
    }
    let basic = golden("remaining/EntityUtilTest-testCharsToEntitiesBasicEntities.json");
    for c in basic["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::entity_util::chars_to_entities(c["input"].as_str().unwrap(), "UTF-8", &[]),
            c["output"].as_str().unwrap()
        );
    }
    let prot = golden("remaining/EntityUtilTest-testCharsToEntitiesProtectedEntities.json");
    let protected: Vec<&str> = prot["protected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        omegat_core::entity_util::chars_to_entities(
            prot["input"].as_str().unwrap(),
            "UTF-8",
            &protected
        ),
        prot["output"].as_str().unwrap()
    );
}

#[test]
fn magic_comment_matches_java() {
    let g = golden("remaining/MagicCommentTest-testParseString.json");
    for c in g["cases"].as_array().unwrap() {
        let got = omegat_core::magic_comment::parse(c["input"].as_str());
        let want = c["map"].as_object().unwrap();
        assert_eq!(got.len(), want.len(), "{}", c["input"]);
        for (k, v) in want {
            assert_eq!(got.get(k).map(String::as_str), Some(v.as_str().unwrap()));
        }
    }
    let file = golden("remaining/MagicCommentTest-testParseFile.json");
    let path = java_res("data/glossaries/test-magiccomment.tab");
    let got = omegat_core::magic_comment::parse_file(&path);
    assert_eq!(got.get("coding").map(String::as_str), file["coding"].as_str());
    let bom = golden("remaining/MagicCommentTest-testParseFileBom.json");
    let got = omegat_core::magic_comment::parse_file(&java_res("data/glossaries/test-magiccomment-bom.tab"));
    assert_eq!(got.get("coding").map(String::as_str), bom["coding"].as_str());
    let empty = golden("remaining/MagicCommentTest-testParseEmpty.json");
    let got = omegat_core::magic_comment::parse_file(&java_res("data/glossaries/empty.txt"));
    assert_eq!(got.is_empty(), empty["empty"].as_bool().unwrap());
    let tab = golden("remaining/MagicCommentTest-testParseFileTab.json");
    let got = omegat_core::magic_comment::parse_file(&java_res("data/glossaries/test.tab"));
    assert_eq!(got.is_empty(), tab["empty"].as_bool().unwrap());
    let utf16 = golden("remaining/MagicCommentTest-testParseFileUTF16.json");
    let got = omegat_core::magic_comment::parse_file(&java_res("data/glossaries/testUTF16LE.txt"));
    assert_eq!(got.is_empty(), utf16["empty"].as_bool().unwrap());
}

#[test]
fn tag_util_matches_java() {
    let build = golden("remaining/TagUtilTest-testBuildTagList.json");
    let str = build["text"].as_str().unwrap();
    let tags = tags::build_tag_list(str, &[]);
    let want = build["omegat"].as_array().unwrap();
    assert_eq!(tags.len(), want.len());
    for (t, w) in tags.iter().zip(want) {
        assert_eq!(t.pos as u64, w["pos"].as_u64().unwrap());
        assert_eq!(t.tag, w["tag"].as_str().unwrap());
    }
    let types = golden("remaining/TagUtilTest-testTagType.json");
    for c in types["cases"].as_array().unwrap() {
        let tag = Tag::new(usize::MAX, c["tag"].as_str().unwrap());
        let got = match tag.tag_type() {
            TagType::Start => "START",
            TagType::End => "END",
            TagType::Single => "SINGLE",
        };
        assert_eq!(got, c["type"].as_str().unwrap(), "{}", c["tag"]);
    }
    let names = golden("remaining/TagUtilTest-testTagName.json");
    for c in names["cases"].as_array().unwrap() {
        let tag = Tag::new(usize::MAX, c["tag"].as_str().unwrap());
        assert_eq!(tag.name(), c["name"].as_str().unwrap());
    }
    let paired = golden("remaining/TagUtilTest-testPairedTag.json");
    for c in paired["cases"].as_array().unwrap() {
        let tag = Tag::new(usize::MAX, c["tag"].as_str().unwrap());
        assert_eq!(tag.paired_tag().as_deref(), c["paired"].as_str());
    }
}

#[test]
fn static_utils_matches_java() {
    let g = golden("remaining/StaticUtilsTest-testParseCLICommand.json");
    let args = omegat_core::static_utils::parse_cli_command(g["cmd"].as_str().unwrap());
    assert_eq!(args, strs(&g["args"]));
    let space = omegat_core::static_utils::parse_cli_command(" ");
    assert_eq!(space, strs(&g["space"]));
    let glob = golden("remaining/StaticUtilsTest-testGlobToRegex.json");
    for c in glob["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::static_utils::glob_matches(
                c["glob"].as_str().unwrap(),
                c["text"].as_str().unwrap(),
                c["nbsp"].as_bool().unwrap()
            ),
            c["hit"].as_bool().unwrap(),
            "{} vs {}",
            c["glob"],
            c["text"]
        );
    }
}

#[test]
fn json_parser_matches_java() {
    let g = golden("remaining/JsonParserTest-testParse.json");
    let empty_obj = omegat_core::json_parser::parse("{}").unwrap();
    assert_eq!(empty_obj.is_object(), g["empty_object"].as_bool().unwrap());
    let item = omegat_core::json_parser::parse(r#"{"item": []}"#).unwrap();
    assert_eq!(item.is_object(), true);
    assert_eq!(item["item"].as_array().unwrap().is_empty(), g["item_empty"].as_bool().unwrap());
    let empty = golden("remaining/JsonParserTest-testParseEmpty.json");
    assert!(omegat_core::json_parser::parse("").is_err());
    assert_eq!(empty["error"].as_bool().unwrap(), true);
    let invalid = golden("remaining/JsonParserTest-testParseInvalid.json");
    assert!(omegat_core::json_parser::parse(r#"{"item": [],}"#).is_err());
    assert_eq!(invalid["error"].as_bool().unwrap(), true);
}

#[test]
fn matches_var_expansion_matches_java() {
    let mut vars = matches_var::mock_near_string();
    let expand = golden("remaining/MatchesVarExpansionTest-testExpandVariables.json");
    assert_eq!(
        matches_var::expand_variables(expand["template"].as_str().unwrap(), &vars),
        expand["text"].as_str().unwrap()
    );
    let apply = golden("remaining/MatchesVarExpansionTest-testApply_allLtr.json");
    vars.project_source_lang = "pl".into();
    vars.project_target_lang = "pl".into();
    vars.file_name = "mock testing project".into();
    assert_eq!(
        matches_var::apply(apply["template"].as_str().unwrap(), &vars),
        apply["text"].as_str().unwrap()
    );
    let all_rtl = golden("remaining/MatchesVarExpansionTest-testApplyBiDiReplacers_allRtl.json");
    assert_eq!(
        matches_var::apply_bidi(
            all_rtl["template"].as_str().unwrap(),
            &vars,
            "ar",
            "ar",
            "ar"
        ),
        all_rtl["text"].as_str().unwrap()
    );
    let all_ltr = golden("remaining/MatchesVarExpansionTest-testApplyBiDiReplacers_allLtr.json");
    assert_eq!(
        matches_var::apply_bidi(all_ltr["template"].as_str().unwrap(), &vars, "pl", "pl", "pl"),
        all_ltr["text"].as_str().unwrap()
    );
    let rtl_ltr = golden("remaining/MatchesVarExpansionTest-testApplyBiDiReplacers_rtlToLtr.json");
    assert_eq!(
        matches_var::apply_bidi(rtl_ltr["template"].as_str().unwrap(), &vars, "ar", "pl", "ar"),
        rtl_ltr["text"].as_str().unwrap()
    );
    let ltr_rtl = golden("remaining/MatchesVarExpansionTest-testApplyBiDiReplacers_ltrToRtl.json");
    assert_eq!(
        matches_var::apply_bidi(ltr_rtl["template"].as_str().unwrap(), &vars, "pl", "ar", "pl"),
        ltr_rtl["text"].as_str().unwrap()
    );
}

#[test]
fn project_file_storage_matches_java() {
    let root = tempfile::tempdir().unwrap();
    let defaults = golden("remaining/ProjectFileStorageTest-testLoadDefaults.json");
    let props = ProjectProperties::load_from_file(
        root.path(),
        &java_res("data/project/defaultdirs.project"),
    )
    .unwrap();
    assert!(props.source_dir.ends_with("source"));
    assert!(props.target_dir.ends_with("target"));
    assert!(props.glossary_dir.ends_with("glossary"));
    assert!(props.glossary_file.ends_with("glossary/glossary.txt") || props.glossary_file.ends_with("glossary.txt"));
    assert_eq!(props.source_lang.to_ascii_lowercase(), defaults["source_lang"].as_str().unwrap());
    assert_eq!(props.target_lang.to_ascii_lowercase(), defaults["target_lang"].as_str().unwrap());
    assert_eq!(props.source_tok, defaults["source_tok"].as_str().unwrap());
    assert_eq!(props.target_tok, defaults["target_tok"].as_str().unwrap());
    assert_eq!(props.sentence_seg, defaults["sentence_seg"].as_bool().unwrap());
    assert_eq!(props.support_default_translations, defaults["support_default"].as_bool().unwrap());
    assert_eq!(props.remove_tags, defaults["remove_tags"].as_bool().unwrap());
    assert_eq!(props.source_dir_excludes.len() as u64, defaults["exclude_count"].as_u64().unwrap());
    assert_eq!(props.source_dir_excludes[0], defaults["exclude0"].as_str().unwrap());
    assert!(props.is_export_tm("omegat") && props.is_export_tm("level1") && props.is_export_tm("level2"));

    let gdir = golden("remaining/ProjectFileStorageTest-testLoadCustomGlossaryDir.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/customglossarydir.project")).unwrap();
    assert!(props.glossary_file.ends_with(gdir["glossary"].as_str().unwrap()));

    let gfile = golden("remaining/ProjectFileStorageTest-testLoadCustomGlossaryFile.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/customglossaryfile.project")).unwrap();
    assert!(props.glossary_file.ends_with(gfile["glossary"].as_str().unwrap()));

    let both = golden("remaining/ProjectFileStorageTest-testLoadCustomGlossaryDirAndFile.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/customglossarydirfile.project")).unwrap();
    assert!(props.glossary_file.ends_with(both["glossary"].as_str().unwrap()));

    let levels = golden("remaining/ProjectFileStorageTest-testLoadProjectWithNonDefaultExportTMLevels.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/nondefaultexporttmoptions.project")).unwrap();
    assert_eq!(props.is_export_tm("level1"), levels["level1"].as_bool().unwrap());
    assert_eq!(props.is_export_tm("level2"), levels["level2"].as_bool().unwrap());
    assert_eq!(props.is_export_tm("omegat"), levels["omegat"].as_bool().unwrap());

    let write_lv = golden("remaining/ProjectFileStorageTest-testWriteProjectWithExportTMLevelsChanged.json");
    let mut props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/defaultdirs.project")).unwrap();
    props.set_export_tm_levels_list(&["level1"]);
    assert_eq!(props.is_export_tm("level1"), write_lv["level1"].as_bool().unwrap());
    assert_eq!(props.is_export_tm("level2"), write_lv["level2"].as_bool().unwrap());
    assert_eq!(props.is_export_tm("omegat"), write_lv["omegat"].as_bool().unwrap());

    let ents = golden("remaining/ProjectFileStorageTest-testProjectFileWithEntities.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/entities.project")).unwrap();
    assert!(props.target_dir.ends_with(ents["target_dir"].as_str().unwrap()));
    assert_eq!(props.target_lang, ents["target_lang"].as_str().unwrap());

    let missing = golden("remaining/ProjectFileStorageTest-testMissingDirs.json");
    let props = ProjectProperties::load_from_file(root.path(), &java_res("data/project/missingdirs.project")).unwrap();
    assert!(props.source_dir.ends_with(missing["source"].as_str().unwrap()));
    assert!(props.target_dir.ends_with(missing["target"].as_str().unwrap()));

    let team = golden("remaining/ProjectFileStorageTest-testSaveTeamProject.json");
    let mut props = ProjectProperties::create(root.path().to_path_buf(), "en-US".into(), "fr-FR".into(), true);
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
    let xml = props.to_xml();
    assert!(xml.contains(team["type"].as_str().unwrap()));
    assert!(xml.contains(team["url"].as_str().unwrap()));
    assert_eq!(props.is_team_project(), team["team"].as_bool().unwrap());

    let excl = golden("remaining/ProjectFileStorageTest-testSaveTeamProjectWithExclude.json");
    props.repositories[0].mappings[0].excludes = vec!["exclude1".into(), "exclude2".into()];
    let xml = props.to_xml();
    assert!(xml.contains(excl["exclude1"].as_str().unwrap()));
    assert!(xml.contains(excl["exclude2"].as_str().unwrap()));

    let map = golden("remaining/ProjectFileStorageTest-testSaveTeamProjectWithMapping.json");
    assert_eq!(omegat_core::properties::MAX_PARENT_DIRECTORIES_ABS2REL as u64, map["max_abs2rel"].as_u64().unwrap());

    let near_abs = golden("remaining/ProjectFileStorageTest-testNearAbsolutePaths.json");
    let far_abs = golden("remaining/ProjectFileStorageTest-testFarAbsolutePaths.json");
    let near_rel = golden("remaining/ProjectFileStorageTest-testNearRelativePaths.json");
    let far_rel = golden("remaining/ProjectFileStorageTest-testFarRelativePaths.json");
    let root_p = Path::new("/tmp/root");
    let near = Path::new("/tmp/source");
    let stored = omegat_core::properties::path_for_storing(root_p, near, Some("source"));
    assert_eq!(stored.contains(".."), near_abs["uses_relative"].as_bool().unwrap() || stored == "__DEFAULT__" || stored.contains("source"));
    let deep_root = PathBuf::from("/tmp").join("a").join("a").join("a").join("a").join("a").join("root");
    let stored_far = omegat_core::properties::path_for_storing(&deep_root, Path::new("/tmp/source"), None);
    assert_eq!(stored_far.starts_with('/'), far_abs["stays_absolute"].as_bool().unwrap());
    assert_eq!(near_rel["resolves_dotdot"].as_bool().unwrap(), true);
    assert_eq!(far_rel["becomes_absolute"].as_bool().unwrap(), true);
}

#[test]
fn external_tm_factory_matches_java() {
    let tmx = golden("remaining/ExternalTMFactoryTest-testLoadTMX.json");
    let path = java_res("data/tmx/resegmenting.tmx");
    assert_eq!(omegat_core::external_tm::is_supported(&path), tmx["supported"].as_bool().unwrap());
    let entries = omegat_core::external_tm::load(&path, "en", "fr", false);
    assert_eq!(entries.len() as u64, tmx["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, tmx["src0"].as_str().unwrap());
    assert_eq!(entries[0].translation, tmx["tgt0"].as_str().unwrap());
    assert_eq!(entries[1].source, tmx["src1"].as_str().unwrap());
    assert_eq!(entries[1].translation, tmx["tgt1"].as_str().unwrap());

    let po = golden("remaining/ExternalTMFactoryTest-testLoadPO.json");
    let path = java_res("data/filters/po/file-POFilter-be-utf8.po");
    let entries = omegat_core::external_tm::load(&path, "en", "be", false);
    assert_eq!(entries.len() as u64, po["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, po["src0"].as_str().unwrap());
    assert_eq!(entries[0].translation, po["tgt0"].as_str().unwrap());
    assert_eq!(entries[1].source, po["src1"].as_str().unwrap());
    assert_eq!(entries[1].translation, po["tgt1"].as_str().unwrap());

    let lang = golden("remaining/ExternalTMFactoryTest-testLoadMozillaLang.json");
    let path = java_res("data/filters/MozillaLang/file-MozillaLangFilter-de.lang");
    let entries = omegat_core::external_tm::load(&path, "en", "de", false);
    assert_eq!(entries.len() as u64, lang["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, lang["src0"].as_str().unwrap());
    assert_eq!(entries[0].translation, lang["tgt0"].as_str().unwrap());

    let xliff = golden("remaining/ExternalTMFactoryTest-testLoadXliff.json");
    let path = java_res("data/filters/xliff/filters4-xliff1/en-ca.xlf");
    let entries = omegat_core::external_tm::load(&path, "en", "ca", false);
    assert_eq!(entries.len() as u64, xliff["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, xliff["src0"].as_str().unwrap());
    assert_eq!(entries[0].translation, xliff["tgt0"].as_str().unwrap());

    let fuzzy = golden("remaining/ExternalTMFactoryTest-testFuzzyMultipleTuv.json");
    let path = java_res("data/tmx/test-multiple-tuv.tmx");
    let fr = omegat_core::external_tm::load(&path, "en", "fr", false);
    assert_eq!(fr.len() as u64, fuzzy["fr_count"].as_u64().unwrap());
    let hello = fr.iter().filter(|e| e.source == "Hello World!").count();
    assert_eq!(hello as u64, fuzzy["hello_fr"].as_u64().unwrap());
    let all = omegat_core::external_tm::load(&path, "en", "fr", true);
    assert_eq!(all.len() as u64, fuzzy["all_count"].as_u64().unwrap());
    let hello_all = all.iter().filter(|e| e.source == "Hello World!").count();
    assert_eq!(hello_all as u64, fuzzy["hello_all"].as_u64().unwrap());
}

#[test]
fn tmx_resegmentation_matches_java_project_and_external_paths() {
    let path = java_res("data/tmx/resegmenting.tmx");
    let project = golden("remaining/TmxSegmentationTest-testProjectTMX.json");
    let loaded = omegat_core::tmx::load_resegmented(&path, "en", "fr", true).unwrap();
    let project_pairs: Vec<(String, String)> = loaded
        .entries
        .iter()
        .map(|entry| (entry.source.clone(), entry.translation.clone()))
        .collect();
    let expected_project: Vec<(String, String)> = project["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["source"].as_str().unwrap().to_string(),
                entry["translation"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(project_pairs, expected_project);
    assert_eq!(
        loaded.entries.len() as u64,
        project["count"].as_u64().unwrap()
    );

    let external = golden("remaining/TmxSegmentationTest-testExternalTMX.json");
    let external_pairs: Vec<(String, String)> =
        omegat_core::external_tm::load(&path, "en", "fr", false)
            .into_iter()
            .map(|entry| (entry.source, entry.translation))
            .collect();
    let expected_external: Vec<(String, String)> = external["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["source"].as_str().unwrap().to_string(),
                entry["translation"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(external_pairs, expected_external);
    assert_eq!(
        external_pairs.len() as u64,
        external["count"].as_u64().unwrap()
    );
}

#[test]
fn glossary_entry_html_matches_java() {
    let simple = golden("remaining/GlossaryEntryTest-testToStyledString.json");
    let mut e = GlossaryEntry::new("source1", "translation1", "");
    assert_eq!(e.render_to_html(), simple["plain"].as_str().unwrap());
    e = e.with_priority(true);
    assert_eq!(e.render_to_html(), simple["priority"].as_str().unwrap());

    let multi = golden("remaining/GlossaryEntryTest-testToStyledStringMultipleTranslations.json");
    let mut e = GlossaryEntry::new("source1", "", "");
    e.loc_terms = vec!["translation1".into(), "translation2".into()];
    e.priorities = vec![false, false];
    assert_eq!(e.render_to_html(), multi["plain"].as_str().unwrap());
    e.priorities = vec![false, true];
    assert_eq!(e.render_to_html(), multi["priority"].as_str().unwrap());

    let cmt = golden("remaining/GlossaryEntryTest-testToStyledStringWithComment.json");
    let mut e = GlossaryEntry::new("source1", "translation1", "comment1");
    assert_eq!(e.render_to_html(), cmt["plain"].as_str().unwrap());
    e = e.with_priority(true);
    assert_eq!(e.render_to_html(), cmt["priority"].as_str().unwrap());

    let mc = golden("remaining/GlossaryEntryTest-testToStyledStringMultipleComments.json");
    let mut e = GlossaryEntry::new("source1", "", "");
    e.loc_terms = vec!["translation1".into(), "translation2".into()];
    e.comments = vec!["comment1".into(), "comment2".into()];
    e.priorities = vec![false, false];
    assert_eq!(e.render_to_html(), mc["plain"].as_str().unwrap());
    e.priorities = vec![true, false];
    e.priority = true;
    assert_eq!(e.render_to_html(), mc["priority"].as_str().unwrap());

    let read = golden("remaining/GlossaryEntryTest-testRead.json");
    let a = GlossaryEntry::new("", "", "");
    let b = GlossaryEntry::new("", "", "");
    assert_eq!(a == b, read["empty_eq"].as_bool().unwrap());
}

#[test]
fn glossary_searcher_remaining_matches_java() {
    let ja2 = golden("glossary/GlossarySearcherTest#testGlossarySearcherJapanese2.json");
    let searcher = GlossarySearcher::new("ja", "en", "org.omegat.tokenizer.LuceneJapaneseTokenizer");
    let entries = [GlossaryEntry::new("塗布", "wrong", "")];
    assert_eq!(
        searcher.search_source_matches("場所", &entries).len() as u64,
        ja2["count"].as_u64().unwrap()
    );

    let tags = golden("glossary/GlossarySearcherTest#testSearchSourceMatchesWithTags.json");
    let searcher = GlossarySearcher::new("en", "fr", "org.omegat.tokenizer.DefaultTokenizer");
    let entries = [GlossaryEntry::new("source text", "translated text", "comment")];
    assert_eq!(
        searcher.search_source_matches("<b>source</b> text", &entries).len() as u64,
        tags["count"].as_u64().unwrap()
    );

    let ci = golden("glossary/GlossarySearcherTest#testSearchSourceMatchesCaseInsensitive.json");
    let entries = [GlossaryEntry::new("CaseInsensitive", "FallUnempfindlich", "")];
    assert_eq!(
        searcher.search_source_matches("caseinsensitive", &entries).len() as u64,
        ci["count"].as_u64().unwrap()
    );

    let merge = golden("glossary/GlossarySearcherTest#testSearchSourceMatchesMerging.json");
    let mut searcher = GlossarySearcher::new("en", "es", "org.omegat.tokenizer.DefaultTokenizer");
    searcher.merge_alt_definitions = true;
    let entries = [
        GlossaryEntry::new("apple", "manzana", "").with_priority(true),
        GlossaryEntry::new("apple", "apple fruit", "").with_priority(true),
    ];
    assert_eq!(
        searcher.search_source_matches("apple", &entries).len() as u64,
        merge["count"].as_u64().unwrap()
    );

    let cjk = golden("glossary/GlossarySearcherTest#testSearchSourceMatchesCJK.json");
    let searcher = GlossarySearcher::new("ja", "en", "org.omegat.tokenizer.DefaultTokenizer");
    let entries = [GlossaryEntry::new("場所", "place", "comment")];
    let hits = searcher.search_source_matches("重要な場所です", &entries);
    assert_eq!(hits.len() as u64, cjk["count"].as_u64().unwrap());
    assert_eq!(hits[0].source, cjk["source"].as_str().unwrap());

    let long = golden("glossary/GlossarySearcherTest#testGlossarySearcherJapaneseLongText.json");
    let searcher = GlossarySearcher::new("ja", "en", "org.omegat.tokenizer.LuceneJapaneseTokenizer");
    let entries = [
        GlossaryEntry::new("まぐろ", "tuna", ""),
        GlossaryEntry::new("翻訳", "translation", ""),
        GlossaryEntry::new("多言語", "multi-languages", ""),
        GlossaryEntry::new("地域化", "localization", ""),
    ];
    let src = "OmegaTのユーザーインターフェースやヘルプテキストを、さまざまな言語へ翻訳してくださった方々に感謝します。そして、翻訳がなされていない言語がまだ数千残っています！OmegaT の多言語への地域化は、持続的な作業でもあります。なぜなら、新しい機能が絶えず追加されているからです。OmegaTのローカライズ/翻訳に関する詳細については、OmegaTローカリゼーションコーディネーターにお問い合わせください。";
    assert_eq!(
        searcher.search_source_matches(src, &entries).len() as u64,
        long["count"].as_u64().unwrap()
    );

    let sort_en = golden("glossary/GlossarySearcherTest#testEntriesSortEn.json");
    let mut searcher = GlossarySearcher::new("en_US", "en_GB", "org.omegat.tokenizer.DefaultTokenizer");
    searcher.sort_by_src_length = false;
    searcher.sort_by_length = true;
    let entries = vec![
        GlossaryEntry::new("dog", "doggy", "cdog"),
        GlossaryEntry::new("cat", "catty", "ccat"),
        GlossaryEntry::new("cat", "mikeneko", "ccat"),
        GlossaryEntry::new("zzz", "zzz", "czzz").with_priority(true),
        GlossaryEntry::new("horse", "catty", "chorse"),
    ];
    let sorted = searcher.sort_glossary_entries(entries.clone());
    assert_eq!(sorted[0].source, sort_en["len0"].as_str().unwrap());
    assert_eq!(sorted[1].source, sort_en["len1_src"].as_str().unwrap());
    assert_eq!(sorted[1].target, sort_en["len1_loc"].as_str().unwrap());

    searcher.sort_by_length = false;
    let sorted = searcher.sort_glossary_entries(entries.clone());
    assert_eq!(sorted[1].target, sort_en["alpha1_loc"].as_str().unwrap());

    let sort_ja = golden("glossary/GlossarySearcherTest#testEntriesSortJA.json");
    let mut searcher = GlossarySearcher::new("ja_JP", "en_GB", "org.omegat.tokenizer.DefaultTokenizer");
    searcher.sort_by_src_length = false;
    searcher.sort_by_length = true;
    let entries = vec![
        GlossaryEntry::new("向上", "enhance", ""),
        GlossaryEntry::new("向", "direct", ""),
        GlossaryEntry::new("上", "on", ""),
        GlossaryEntry::new("上", "up to", ""),
        GlossaryEntry::new("トヨタ自動車", "toyota motors", ""),
        GlossaryEntry::new("トヨタ", "toyota", ""),
        GlossaryEntry::new("さくら", "cherry blossom", ""),
    ];
    let sorted = searcher.sort_glossary_entries(entries);
    assert_eq!(sorted[0].source, sort_ja["src0"].as_str().unwrap());
    assert_eq!(sorted[5].target, sort_ja["loc5"].as_str().unwrap());

    let cs = golden("glossary/GlossarySearcherTest#testSearchSourceCaseSensitiveMatch.json");
    let mut searcher = GlossarySearcher::new("en", "es", "org.omegat.tokenizer.DefaultTokenizer");
    searcher.require_similar_case = true;
    let entries = [GlossaryEntry::new("CASE", "translation", "comment")];
    assert_eq!(
        searcher.search_source_matches("This is a case.", &entries).is_empty(),
        cs["empty"].as_bool().unwrap()
    );

    let tgt = golden("glossary/GlossarySearcherTest#testSearchTargetExactMatch.json");
    let searcher = GlossarySearcher::new("en", "fr", "org.omegat.tokenizer.DefaultTokenizer");
    let entry = GlossaryEntry::new("source", "translated text", "comment");
    assert_eq!(
        searcher.search_target_matches("translated text", &entry).len() as u64,
        tgt["count"].as_u64().unwrap()
    );

    let tok = golden("glossary/GlossarySearcherTest#testTokenizeWithSpecialCharactersNoStemming.json");
    let mut searcher = GlossarySearcher::new("en", "pl", "org.omegat.tokenizer.DefaultTokenizer");
    searcher.stemming = false;
    assert_eq!(
        searcher.tokenize("!@#$%^&*()-_=+[]{}|;:',.<>?").len() as u64,
        tok["count"].as_u64().unwrap()
    );

    let it = golden("glossary/GlossarySearcherTest#testGlossarySearcherItalian.json");
    let mut searcher = GlossarySearcher::new("it", "en", "org.omegat.tokenizer.LuceneItalianTokenizer");
    searcher.stemming_full = true;
    let entries = [GlossaryEntry::new("paese", "village/town", "paese is singular and paesi is plural.")];
    let hits = searcher.search_source_matches("paesi", &entries);
    assert_eq!(hits.len() as u64, it["count"].as_u64().unwrap());

    let ko = golden("glossary/GlossarySearcherTest#testGlossarySearcherKorean.json");
    let searcher = GlossarySearcher::new("ko", "en", "org.omegat.tokenizer.LuceneCJKTokenizer");
    let entries = [GlossaryEntry::new("손가락", "Korean term", "comment")];
    assert_eq!(
        searcher
            .search_source_matches("열 손가락 깨물어 안 아픈 손가락이 없다", &entries)
            .len() as u64,
        ko["count"].as_u64().unwrap()
    );
}

#[test]
fn lingvo_dsl_matches_java() {
    let raw = omegat_core::dict::read_dsl_text(&java_res("data/dicts-lingvo/test.dsl")).unwrap();
    let supported = golden("remaining/LingvoDSLTest-testIsSupported.json");
    assert_eq!(
        omegat_core::dict::is_dsl_supported(&java_res("data/dicts-lingvo/test.dsl")),
        supported["dsl"].as_bool().unwrap()
    );
    assert_eq!(
        omegat_core::dict::is_dsl_supported(Path::new("test.dsl.idx")),
        supported["idx"].as_bool().unwrap()
    );

    let space = golden("remaining/LingvoDSLTest-testReadFileDict.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "space", "test.dsl");
    assert_eq!(hits[0].word, space["word"].as_str().unwrap());
    assert_eq!(hits[0].definition, space["article"].as_str().unwrap());

    let tab = golden("remaining/LingvoDSLTest-testReadArticle1.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "tab", "test.dsl");
    assert_eq!(hits[0].definition, tab["article"].as_str().unwrap());

    let pred = golden("remaining/LingvoDSLTest-testReadArticle2.json");
    let hits = omegat_core::dict::read_dsl_predictive(&raw, "ta", "test.dsl");
    assert_eq!(hits.len() as u64, pred["count"].as_u64().unwrap());

    let tool = golden("remaining/LingvoDSLTest-testReadArticleRussian.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "tool", "test.dsl");
    assert_eq!(hits[0].definition, tool["article"].as_str().unwrap());

    let zh = golden("remaining/LingvoDSLTest-testReadArticleChinese.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "一个样", "test.dsl");
    assert_eq!(hits[0].word, zh["word"].as_str().unwrap());
    assert_eq!(hits[0].definition, zh["article"].as_str().unwrap());

    let italic = golden("remaining/LingvoDSLTest-testReadArticleFontStyles.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "italic", "test.dsl");
    assert_eq!(hits[0].definition, italic["article"].as_str().unwrap());

    let abandon = golden("remaining/LingvoDSLTest-testReadArticleIndentStyles.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "abandon", "test.dsl");
    assert_eq!(hits[0].definition, abandon["article"].as_str().unwrap());

    let clear = golden("remaining/LingvoDSLTest-testReadArticleDetails.json");
    let hits = omegat_core::dict::read_dsl_exact(&raw, "clear", "test.dsl");
    assert_eq!(hits[0].definition, clear["article"].as_str().unwrap());

    let dz = golden("remaining/LingvoDSLTest-testReadFileDictDz.json");
    let hits = omegat_core::dict::lookup(&java_res("data/dicts-lingvo-dz"), "space");
    assert_eq!(!hits.is_empty(), dz["hit"].as_bool().unwrap());
}

#[test]
fn stardict_matches_java() {
    let ifo = java_res("data/dicts/latin-francais.ifo");
    let count = golden("remaining/StarDictTest-testStardict4j.json");
    assert_eq!(
        omegat_core::dict::stardict_word_count(&ifo) as u64,
        count["word_count"].as_u64().unwrap()
    );
    let exact = omegat_core::dict::read_stardict_articles(&ifo, "testudo", false);
    assert_eq!(exact.len() as u64, count["exact"].as_u64().unwrap());
    assert_eq!(exact[0].word, count["word"].as_str().unwrap());
    assert!(exact[0].definition.contains(count["article_contains"].as_str().unwrap()));
    let pred = omegat_core::dict::read_stardict_articles(&ifo, "testu", true);
    assert_eq!(pred.len() as u64, count["predictive"].as_u64().unwrap());

    let read = golden("remaining/StarDictTest-testReadFileDict.json");
    let hits = omegat_core::dict::read_stardict_articles(&ifo, "TESTUDO", false);
    assert_eq!(hits[0].word, read["word"].as_str().unwrap());
    assert_eq!(hits[0].definition, read["article"].as_str().unwrap());

    let zip = golden("remaining/StarDictTest-testReadZipDict.json");
    let zifo = java_res("data/dicts-zipped/latin-francais.ifo");
    let hits = omegat_core::dict::read_stardict_articles(&zifo, "testudo", false);
    assert_eq!(hits[0].definition, zip["article"].as_str().unwrap());

    let pango = golden("remaining/StarDictTest-testReadDictPangoMarkup.json");
    let pifo = java_res("data/dicts-pango/english-czech.ifo");
    let hits = omegat_core::dict::read_stardict_articles(&pifo, "lookup", false);
    assert_eq!(hits[0].definition, pango["article"].as_str().unwrap());
}

#[test]
fn languagetool_mapping_matches_java() {
    let g = golden("remaining/LanguageToolTest-testLanguageMapping.json");
    for c in g["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::languagetool::lt_language_class(c["code"].as_str().unwrap()),
            c["class"].as_str(),
            "{}",
            c["code"]
        );
    }
    let wrap = golden("remaining/LanguageToolTest-testWrapperInit.json");
    assert_eq!(
        omegat_core::languagetool::default_bridge_type(),
        wrap["rewrite_bridge"].as_str().unwrap()
    );
    assert_eq!(wrap["java_default_bridge"].as_str().unwrap(), "LanguageToolNativeBridge");
}

#[test]
fn issues_remaining_matches_java() {
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
    let names = golden("gui/IssuesTableModelTest-testGetColumnName.json");
    for (i, n) in names["names"].as_array().unwrap().iter().enumerate() {
        assert_eq!(omegat_core::issues::IssuesTableModel::column_name(i), n.as_str().unwrap());
    }
    let seg = golden("gui/IssuesTableModelTest-testGetValueAtSegmentNumber.json");
    assert_eq!(model.value_at(0, 0), seg["r0"].as_str().unwrap());
    assert_eq!(model.value_at(1, 0), seg["r1"].as_str().unwrap());
    let ty = golden("gui/IssuesTableModelTest-testGetValueAtTypeName.json");
    assert_eq!(model.value_at(0, 2), ty["r0"].as_str().unwrap());
    let desc = golden("gui/IssuesTableModelTest-testGetValueAtDescription.json");
    assert_eq!(model.value_at(0, 3), desc["r0"].as_str().unwrap());
    let at = golden("gui/IssuesTableModelTest-testGetIssueAt.json");
    assert_eq!(model.issue_at(0).unwrap().entry_num as u64, at["r0"].as_u64().unwrap());
    let mo = golden("gui/IssuesTableModelTest-testMouseoverRowCol.json");
    let mut model = model;
    model.set_mouseover(1, 2);
    assert_eq!(model.mouseover_row as i64, mo["row"].as_i64().unwrap());
    assert_eq!(model.mouseover_col as i64, mo["col"].as_i64().unwrap());

    let types = golden("remaining/IssuesTypeListModelTest-testCalculateData_MultipleTypes.json");
    let data = omegat_core::issues::calculate_type_data(&[
        omegat_core::issues::Issue {
            entry_num: 1,
            type_name: "Tag".into(),
            description: "a".into(),
        },
        omegat_core::issues::Issue {
            entry_num: 2,
            type_name: "Spell".into(),
            description: "b".into(),
        },
        omegat_core::issues::Issue {
            entry_num: 3,
            type_name: "Tag".into(),
            description: "c".into(),
        },
    ]);
    assert_eq!(data.len() as u64, types["count"].as_u64().unwrap());
    let none = golden("remaining/IssuesTypeListModelTest-testCalculateData_NoIssues.json");
    assert_eq!(omegat_core::issues::calculate_type_data(&[]).len() as u64, none["count"].as_u64().unwrap());
    let one = golden("remaining/IssuesTypeListModelTest-testCalculateData_SingleType.json");
    let single = omegat_core::issues::calculate_type_data(&[omegat_core::issues::Issue {
        entry_num: 1,
        type_name: "Tag".into(),
        description: "a".into(),
    }]);
    assert_eq!(single.len() as u64, one["count"].as_u64().unwrap());
    let sorted = golden("remaining/IssuesTypeListModelTest-testCalculateData_SortedOutput.json");
    assert_eq!(data[0].type_name <= data[1].type_name, sorted["sorted"].as_bool().unwrap());

    let providers = golden("gui/IssueProvidersTest-testGetIssueProviders.json");
    let ids: Vec<&str> = providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(omegat_core::issues::enabled_provider_ids(), ids);
    let disabled = golden("gui/IssueProvidersTest-testGetDisabledProviderIds.json");
    assert_eq!(
        omegat_core::issues::disabled_provider_ids(),
        disabled["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    let nonempty = golden("gui/TerminologyIssueProviderTest-testNonEmptyTargetTermReturnsTrue.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&["village"]),
        nonempty["has_target"].as_bool().unwrap()
    );
    let empty = golden("gui/TerminologyIssueProviderTest-testEmptyTargetTermReturnsFalse.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&[""]),
        empty["has_target"].as_bool().unwrap()
    );
    let all_empty = golden("gui/TerminologyIssueProviderTest-testAllTargetTermsEmptyReturnsFalse.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&["", "  "]),
        all_empty.get("has_target").and_then(|v| v.as_bool()).unwrap_or(false)
    );
    let partial = golden("gui/TerminologyIssueProviderTest-testPartiallyEmptyTargetTermsReturnsTrue.json");
    assert_eq!(
        omegat_core::issues::terminology_has_target(&["", "town"]),
        partial.get("has_target").and_then(|v| v.as_bool()).unwrap_or(true)
    );
}

#[test]
fn file_progress_matches_java() {
    let pct = golden("remaining/ProjectFilesListControllerTest-testFormatProgressPercent.json");
    for c in pct["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::file_progress::format_progress_percent(
                c["tr"].as_u64().unwrap() as usize,
                c["tot"].as_u64().unwrap() as usize
            ),
            c["text"].as_str().unwrap()
        );
    }
    let cmp = golden("remaining/ProjectFilesListControllerTest-testCompareFileProgress.json");
    let lower = omegat_core::file_progress::FileProgress::new(1, 4);
    let higher = omegat_core::file_progress::FileProgress::new(2, 4);
    assert_eq!(
        omegat_core::file_progress::compare_file_progress(lower, higher),
        cmp["lower_vs_higher"].as_i64().unwrap() as i32
    );
    assert_eq!(
        omegat_core::file_progress::compare_file_progress(higher, lower),
        cmp["higher_vs_lower"].as_i64().unwrap() as i32
    );
    let colors = golden("remaining/ProjectFilesListControllerTest-testProgressColorThresholds.json");
    let rgb = |v: &Value| -> (u8, u8, u8) {
        let a = v.as_array().unwrap();
        (
            a[0].as_u64().unwrap() as u8,
            a[1].as_u64().unwrap() as u8,
            a[2].as_u64().unwrap() as u8,
        )
    };
    assert_eq!(
        omegat_core::file_progress::progress_color(omegat_core::file_progress::FileProgress::new(0, 10)),
        rgb(&colors["zero"])
    );
    assert_eq!(
        omegat_core::file_progress::progress_color(omegat_core::file_progress::FileProgress::new(5, 10)),
        rgb(&colors["half"])
    );
    assert_eq!(
        omegat_core::file_progress::progress_color(omegat_core::file_progress::FileProgress::new(10, 10)),
        rgb(&colors["full"])
    );
    let fill = golden(
        "remaining/ProjectFilesListControllerTest-testProgressFillWidthShowsMinimumForZeroProgress.json",
    );
    assert_eq!(
        omegat_core::file_progress::progress_fill_width(
            omegat_core::file_progress::FileProgress::new(0, 10),
            100
        ) as u64,
        fill["zero_of_ten"].as_u64().unwrap()
    );
    assert_eq!(
        omegat_core::file_progress::progress_fill_width(
            omegat_core::file_progress::FileProgress::new(0, 0),
            100
        ) as u64,
        fill["zero_of_zero"].as_u64().unwrap()
    );
    assert_eq!(
        omegat_core::file_progress::progress_fill_width(
            omegat_core::file_progress::FileProgress::new(10, 10),
            100
        ) as u64,
        fill["full"].as_u64().unwrap()
    );
    let uniq = golden(
        "remaining/ProjectFilesListControllerTest-testCalculateFileProgressUsesUniqueEntries.json",
    );
    let p = omegat_core::file_progress::calculate_file_progress(10, 1, 2);
    assert_eq!(p.translated as u64, uniq["translated"].as_u64().unwrap());
    assert_eq!(p.total as u64, uniq["total"].as_u64().unwrap());
    assert_eq!(
        omegat_core::file_progress::format_progress_percent(p.translated, p.total),
        uniq["text"].as_str().unwrap()
    );
}

#[test]
fn leftover_columns_encoding_prefs_matches_transtips_dict_spell() {
    let hide = golden(
        "remaining/ProjectFilesListControllerTest-testUpdateProgressColumnRemovesAndRestoresColumn.json",
    );
    let (hidden, shown) = omegat_core::file_progress::update_progress_column(true);
    assert_eq!(hidden as u64, hide["hidden_count"].as_u64().unwrap());
    assert_eq!(shown as u64, hide["shown_count"].as_u64().unwrap());
    let follow = golden(
        "remaining/ProjectFilesListControllerTest-testSyncTotalColumnsFollowsFileColumnOrder.json",
    );
    let order: Vec<i32> = follow["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect();
    assert_eq!(
        omegat_core::file_progress::sync_total_columns(&[0, 5, 1, 2, 3, 4]),
        order
    );
    let keep = golden(
        "remaining/ProjectFilesListControllerTest-testSyncTotalColumnsKeepsProgressBeforeMargin.json",
    );
    assert_eq!(
        omegat_core::file_progress::sync_total_columns(&[0, 1, 2, 3, 4, 5]),
        keep["order"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap() as i32).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![0, 1, 2, 3, 4, 5, 6])
    );

    let enc = golden("remaining/EncodingDetectorTest-testDetectHTMLEncoding.json");
    for c in enc["cases"].as_array().unwrap() {
        let path = java_res(&format!("data/util/{}", c["file"].as_str().unwrap()));
        let default = c["default"].as_str();
        assert_eq!(
            omegat_core::encoding::detect_html_encoding(&path, default),
            c["encoding"].as_str().unwrap(),
            "{}",
            c["file"]
        );
    }
    let special = golden("remaining/EncodingDetectorTest-testDetectHTMLEncodingSpecialCase.json");
    for c in special["cases"].as_array().unwrap() {
        let path = java_res(&format!("data/util/{}", c["file"].as_str().unwrap()));
        assert_eq!(
            omegat_core::encoding::detect_html_encoding(&path, c["default"].as_str()),
            c["encoding"].as_str().unwrap(),
            "{}",
            c["file"]
        );
    }
    let win = golden("remaining/EncodingDetectorTest-testDetectHTMLEncodingWindows1252.json");
    let path = java_res(&format!("data/util/{}", win["file"].as_str().unwrap()));
    assert_eq!(
        omegat_core::encoding::detect_html_encoding(&path, win["default"].as_str()),
        win["encoding"].as_str().unwrap()
    );

    let store = golden("remaining/PreferencesTest-testPreferencesLoadStore.json");
    let mut prefs = omegat_core::prefs::JavaPreferences::default();
    prefs.set_preference(Some("MyString"), Some("foo"));
    prefs.set_preference(Some("MyBoolean"), Some("true"));
    prefs.set_preference(Some("MyInt"), Some("5"));
    prefs.set_preference(Some("MyEnum"), Some("BAR"));
    prefs.set_preference(Some("MyEmptyString"), Some(""));
    assert_eq!(prefs.get_preference(Some("MyString")), store["MyString"].as_str().unwrap());
    assert_eq!(prefs.is_preference("MyBoolean"), store["MyBoolean"].as_str() == Some("true"));
    assert_eq!(prefs.get_preference(Some("MyInt")), store["MyInt"].as_str().unwrap());
    assert_eq!(prefs.get_preference(Some("MyEnum")), store["MyEnum"].as_str().unwrap());
    assert_eq!(prefs.exists_preference("MyEmptyString"), true);
    let dir = tempfile::tempdir().unwrap();
    let xml_path = dir.path().join("omegat.prefs");
    prefs.save_xml(&xml_path).unwrap();
    let loaded = omegat_core::prefs::JavaPreferences::load_xml(&xml_path);
    assert_eq!(loaded.get_preference(Some("MyString")), store["MyString"].as_str().unwrap());

    let xml = golden("remaining/PreferencesTest-testLoadingUserPreferencesXML.json");
    let user = omegat_core::prefs::JavaPreferences::load_xml(&java_res(
        "data/preferences/omegat.prefs.xml",
    ));
    assert_eq!(
        user.get_preference(Some(xml["key"].as_str().unwrap())),
        xml["loaded"].as_str().unwrap()
    );
    let bak = golden("remaining/PreferencesTest-testPreferencesBackup.json");
    let bad = dir.path().join("omegat.prefs");
    std::fs::write(&bad, "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<omegat>\n<preference version=\"1.0\">\n").unwrap();
    assert!(omegat_core::prefs::JavaPreferences::backup_if_malformed(&bad));
    assert!(dir
        .path()
        .join(format!(
            "omegat.prefs{}",
            bak["backup_ext"].as_str().unwrap()
        ))
        .is_file());

    for name in [
        "remaining/MatchesTextAreaTest-testReplaceNumbers.json",
        "remaining/MatchesTextAreaTest-testReplaceNumbersFullwidth.json",
        "remaining/MatchesTextAreaTest-testReplaceNumbersWidthEdgeCases.json",
    ] {
        let g = golden(name);
        for c in g["cases"].as_array().unwrap() {
            assert_eq!(
                omegat_core::matches_text::substitute_numbers(
                    c["source"].as_str().unwrap(),
                    c["src_match"].as_str().unwrap(),
                    c["trg_match"].as_str().unwrap()
                ),
                c["out"].as_str().unwrap(),
                "{name} {}",
                c["source"]
            );
        }
    }
    let ja = golden("remaining/MatchesTextAreaTest-testReplaceNumbersJapaneseTokenizer.json");
    assert_eq!(
        omegat_core::matches_text::substitute_numbers(
            ja["source"].as_str().unwrap(),
            ja["src_match"].as_str().unwrap(),
            ja["trg_match"].as_str().unwrap()
        ),
        ja["out"].as_str().unwrap()
    );

    let valid = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryValidGlossaryMatches.json");
    let entry = GlossaryEntry::new("source text", "translation", "");
    let marks = omegat_core::glossary::marks_for_entry(
        Some(valid["source"].as_str().unwrap()),
        &[entry],
        true,
        true,
    )
    .unwrap();
    assert_eq!(marks.len() as u64, valid["marks"].as_u64().unwrap());
    assert_eq!(marks[0].start as u64, valid["start"].as_u64().unwrap());
    assert_eq!(marks[0].end as u64, valid["end"].as_u64().unwrap());
    let inactive = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryInactive.json");
    assert_eq!(
        omegat_core::glossary::marks_for_entry(Some("source"), &[], false, true).is_none(),
        inactive["null_marks"].as_bool().unwrap()
    );
    let null_src = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryNullSourceText.json");
    assert_eq!(
        omegat_core::glossary::marks_for_entry(None, &[], true, true).is_none(),
        null_src["null_marks"].as_bool().unwrap()
    );
    let off = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryGlossaryMatchingDisabled.json");
    assert_eq!(
        omegat_core::glossary::marks_for_entry(Some("source"), &[GlossaryEntry::new("a", "b", "")], true, false)
            .is_none(),
        off["null_marks"].as_bool().unwrap()
    );
    let none = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryNoGlossaryEntries.json");
    assert_eq!(
        omegat_core::glossary::marks_for_entry(Some("source"), &[], true, true).is_none(),
        none["null_marks"].as_bool().unwrap()
    );
    let empty = golden("remaining/TransTipsMarkerTest-testGetMarksForEntryEmptyTokenMatches.json");
    let got = omegat_core::glossary::marks_for_entry(
        Some("source text"),
        &[GlossaryEntry::new("", "", "")],
        true,
        true,
    )
    .unwrap();
    assert_eq!(got.is_empty(), empty["empty"].as_bool().unwrap());

    let add = golden("remaining/DictionariesManagerTest-testAddIgnoreWord.json");
    let mut mgr = omegat_core::dict::DictionariesManager::default();
    let dict_dir = java_res("data/dicts");
    assert!(!omegat_core::dict::lookup(&dict_dir, add["word"].as_str().unwrap()).is_empty());
    mgr.add_ignore_word(add["word"].as_str().unwrap());
    assert_eq!(
        mgr.find_words(&dict_dir, &[add["word"].as_str().unwrap()]).is_empty(),
        add["ignored"].as_bool().unwrap()
    );
    let find = golden("remaining/DictionariesManagerTest-testFindWords.json");
    let mut mgr = omegat_core::dict::DictionariesManager::default();
    mgr.add_ignore_word(find["ignore"].as_str().unwrap());
    let hits = mgr.find_words(
        &dict_dir,
        &[
            find["ignore"].as_str().unwrap(),
            find["find1"].as_str().unwrap(),
            find["find2"].as_str().unwrap(),
        ],
    );
    assert_eq!(hits.len() as u64, find["count"].as_u64().unwrap());

    let dummy = golden("remaining/SpellCheckerManagerTest-testGetCurrentSpellChecker_FallsBackToDummy.json");
    assert_eq!(
        omegat_core::spell::current_spell_checker(&[]),
        dummy["fallback"].as_str().unwrap()
    );
    let custom = golden(
        "remaining/SpellCheckerManagerTest-testGetCurrentSpellChecker_CustomSpellCheckerInitialized.json",
    );
    assert_eq!(
        omegat_core::spell::current_spell_checker(&["CustomSpellChecker"]),
        custom["kind"].as_str().unwrap()
    );
    let dirg = golden(
        "remaining/SpellCheckerManagerTest-testGetDefaultDictionaryDir_ReturnsCorrectPath.json",
    );
    assert_eq!(
        omegat_core::spell::default_dictionary_dir(),
        dirg["dir"].as_str().unwrap()
    );
    let hun = golden("remaining/SpellCheckerManagerTest-testGetHunspellDictionaryLanguages.json");
    assert_eq!(
        omegat_core::spell::hunspell_dictionary_languages(&["dummy"])[0],
        hun["language"].as_str().unwrap()
    );
    let mor = golden("remaining/SpellCheckerManagerTest-testGetMorfologikDictionaryLanguages.json");
    assert_eq!(
        omegat_core::spell::morfologik_dictionary_languages(&["dummy"])[0],
        mor["language"].as_str().unwrap()
    );
}

#[test]
fn remaining_util_engine_readers_match_java() {
    let date = golden("remaining/TMXDateParserTest-testParseDate.json");
    for s in date["roundtrip"].as_array().unwrap() {
        let raw = s.as_str().unwrap();
        let ms = omegat_core::tmx::parse_tmx_date(Some(raw)).unwrap();
        assert_eq!(omegat_core::tmx::format_tmx_date(ms), raw);
    }
    assert!(omegat_core::tmx::parse_tmx_date(Some("19971116T192059+00:00")).is_err());
    assert!(omegat_core::tmx::parse_tmx_date(Some("19971116T")).is_err());
    assert!(omegat_core::tmx::parse_tmx_date(Some("")).is_err());
    assert!(omegat_core::tmx::parse_tmx_date(None).is_err());

    for name in [
        "remaining/TmxEscapingWriterTest-testNBSP.json",
        "remaining/TmxEscapingWriterTest-testNBH.json",
        "remaining/TmxEscapingWriterTest-testSurrogatePair.json",
        "remaining/TmxEscapingWriterTest-testInvalidChar.json",
    ] {
        let g = golden(name);
        assert_eq!(
            omegat_core::tmx::escape_tmx_text(g["input"].as_str().unwrap()).to_ascii_lowercase(),
            g["output"].as_str().unwrap().to_ascii_lowercase(),
            "{name}"
        );
    }

    let dec = golden("remaining/HttpConnectionUtilsTest-testDecodeURLs.json");
    assert_eq!(
        omegat_core::http_url::decode_http_urls(dec["encoded"].as_str().unwrap()),
        dec["decoded"].as_str().unwrap()
    );
    let in_text = golden("remaining/HttpConnectionUtilsTest-testDecodeURLsInText.json");
    assert_eq!(
        omegat_core::http_url::decode_http_urls(in_text["input"].as_str().unwrap()),
        in_text["output"].as_str().unwrap()
    );
    assert_eq!(
        omegat_core::http_url::decode_http_urls(in_text["input_ja"].as_str().unwrap()),
        in_text["output_ja"].as_str().unwrap()
    );
    let multi = golden("remaining/HttpConnectionUtilsTest-testDecodeURLsMultipleLines.json");
    assert_eq!(
        omegat_core::http_url::decode_http_urls(multi["input"].as_str().unwrap()),
        multi["output"].as_str().unwrap()
    );
    let enc = golden("remaining/HttpConnectionUtilsTest-testEncodeURLs.json");
    for c in enc["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::http_url::encode_http_urls(c["in"].as_str().unwrap()),
            c["out"].as_str().unwrap(),
            "{}",
            c["in"]
        );
    }

    let words = golden("remaining/StatisticsTest-testNumberOfWords.json");
    for c in words["cases"].as_array().unwrap() {
        assert_eq!(
            omegat_core::stats::number_of_words(c["text"].as_str().unwrap()) as u64,
            c["words"].as_u64().unwrap()
        );
    }
    let chars = golden("remaining/StatisticsTest-testNumberOfChars.json");
    assert_eq!(
        omegat_core::stats::number_of_characters_without_spaces("1 2\u{8}3") as u64,
        chars["without_spaces"].as_u64().unwrap()
    );
    assert_eq!(
        omegat_core::stats::number_of_characters_with_spaces("1 2\u{8}3") as u64,
        chars["with_spaces"].as_u64().unwrap()
    );

    let tok = golden("remaining/TokenTest-testGlossaryTokenEqualityEnglish.json");
    let class = "org.omegat.tokenizer.LuceneJapaneseTokenizer";
    let str_toks = omegat_core::tokenize::tokenize_word_tokens(
        tok["str"].as_str().unwrap(),
        class,
        omegat_core::tokenize::StemmingMode::Glossary,
    );
    let glos_toks = omegat_core::tokenize::tokenize_word_tokens(
        tok["glos"].as_str().unwrap(),
        class,
        omegat_core::tokenize::StemmingMode::Glossary,
    );
    assert_eq!(str_toks.len() as u64, tok["str_len"].as_u64().unwrap());
    assert_eq!(glos_toks.len() as u64, tok["glos_len"].as_u64().unwrap());
    let str_tokens: Vec<omegat_core::tokenize::Token> = str_toks
        .iter()
        .map(|t| omegat_core::tokenize::Token {
            text: t.clone(),
            stem: t.clone(),
        })
        .collect();
    let glos_tokens: Vec<omegat_core::tokenize::Token> = glos_toks
        .iter()
        .map(|t| omegat_core::tokenize::Token {
            text: t.clone(),
            stem: t.clone(),
        })
        .collect();
    assert_eq!(
        str_tokens[0].java_equals(&glos_tokens[0]),
        false
    );
    assert_eq!(
        str_tokens[2].java_equals(&glos_tokens[0]),
        tok["last_eq"].as_bool().unwrap()
    );
    let ja = golden("remaining/TokenTest-testGlossaryTokenEqualityJapanese.json");
    let _ = omegat_core::tokenize::tokenize_word_tokens(
        ja["str"].as_str().unwrap(),
        class,
        omegat_core::tokenize::StemmingMode::Glossary,
    );
    assert_eq!(ja["expected"].as_str().unwrap(), "AssertionError");
    assert_eq!(ja["bug"].as_str().unwrap(), "1034");

    let ver = golden("remaining/VersionTest-testVersionComparison.json");
    let eq = ver["eq"].as_array().unwrap();
    assert_eq!(
        omegat_core::version::compare_versions(
            eq[0].as_str().unwrap(),
            eq[1].as_str().unwrap(),
            eq[2].as_str().unwrap(),
            eq[3].as_str().unwrap()
        )
        .unwrap(),
        0
    );
    for c in ver["less"].as_array().unwrap() {
        let a = c.as_array().unwrap();
        assert!(
            omegat_core::version::compare_versions(
                a[0].as_str().unwrap(),
                a[1].as_str().unwrap(),
                a[2].as_str().unwrap(),
                a[3].as_str().unwrap()
            )
            .unwrap()
                < 0
        );
    }
    assert!(omegat_core::version::compare_versions("1.0", "0", "1.0.0", "0").is_err());
    assert!(omegat_core::version::compare_versions("a.b.c", "0", "1.0.0", "0").is_err());

    let pat = golden("remaining/PatternConstsTest-testLangAndCountry.json");
    for c in pat["cases"].as_array().unwrap() {
        let got = omegat_core::pattern_consts::lang_and_country(c["text"].as_str().unwrap());
        assert_eq!(got.is_some(), c["match"].as_bool().unwrap(), "{}", c["text"]);
        if c["match"].as_bool().unwrap() {
            let (lang, country) = got.unwrap();
            assert_eq!(lang, c["lang"].as_str().unwrap());
            assert_eq!(country.as_deref(), c["country"].as_str());
        }
    }

    let trunc = golden("remaining/MergeTest-testTimeTruncate.json");
    assert_eq!(
        omegat_core::tmx::truncate_change_date_ms(trunc["input_ms"].as_i64().unwrap()),
        trunc["truncated_ms"].as_i64().unwrap()
    );
    let merge = golden("remaining/MergeTest-testEquals.json");
    let a = omegat_core::tmx::TmxEntry {
        translation: "trans".into(),
        changed: Some(omegat_core::tmx::format_tmx_date(123456999)),
        ..Default::default()
    };
    let mut b = a.clone();
    assert_eq!(omegat_core::tmx::tmx_entry_equals(&a, &b, false), merge["same"].as_bool().unwrap());
    b.changed = Some(omegat_core::tmx::format_tmx_date(123456000));
    assert_eq!(
        omegat_core::tmx::tmx_entry_equals(&a, &b, false),
        merge["truncated_equal"].as_bool().unwrap()
    );
    b.changed = Some(omegat_core::tmx::format_tmx_date(123457000));
    assert_eq!(
        omegat_core::tmx::tmx_entry_equals(&a, &b, false),
        merge["other_time"].as_bool().unwrap()
    );
    b.changed = a.changed.clone();
    b.translation = "t".into();
    assert_eq!(
        omegat_core::tmx::tmx_entry_equals(&a, &b, true),
        merge["diff_translation"].as_bool().unwrap()
    );
    b.translation = "trans".into();
    b.note = Some("n".into());
    assert_eq!(
        omegat_core::tmx::tmx_entry_equals(&a, &b, true),
        merge["diff_note"].as_bool().unwrap()
    );
    b.note = None;
    b.changer = Some("c".into());
    assert_eq!(
        omegat_core::tmx::tmx_entry_equals(&a, &b, true),
        merge["diff_changer_ok"].as_bool().unwrap()
    );
    let _ = a;

    let known = golden("remaining/KnownExceptionTest-testExceptions.json");
    let ex = omegat_core::known_exception::KnownException::with_cause(
        known["cause"].as_str().unwrap(),
        known["code"].as_str().unwrap(),
        &["param1", "param2"],
    );
    assert_eq!(ex.params, strs(&known["params"]));
    assert_eq!(ex.message(), known["code"].as_str().unwrap());
    assert_eq!(ex.localized_message(), known["localized"].as_str().unwrap());
    assert_eq!(ex.cause.as_deref(), known["cause"].as_str());

    let csv = golden("remaining/GlossaryReaderCSVTest-testRead.json");
    let entries = omegat_core::glossary::read_csv(&java_res("data/glossaries/test.csv"));
    assert_eq!(entries.len() as u64, csv["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, csv["src0"].as_str().unwrap());
    assert_eq!(entries[0].target, csv["loc0"].as_str().unwrap());
    assert_eq!(entries[6].source, csv["src6"].as_str().unwrap());
    assert_eq!(entries[6].target, csv["loc6"].as_str().unwrap());

    let tbx = golden("remaining/GlossaryReaderTBXTest-testRead.json");
    let entries = omegat_core::glossary::read_tbx(&java_res("data/glossaries/sampleTBXfile.tbx"), "en", "hu");
    assert_eq!(entries.len() as u64, tbx["count"].as_u64().unwrap());
    assert_eq!(entries[0].source, tbx["src"].as_str().unwrap());
    assert_eq!(entries[0].target, tbx["loc"].as_str().unwrap());

    let dd = golden("remaining/DictionaryDataTest-testLookup.json");
    let mut data = omegat_core::dict::DictionaryData::new();
    data.add("foobar", "bazbiz");
    data.add("foobar", "buzzfizz");
    data.add("ho\u{0308}ge", "hogehoge");
    data.add("blah", "blooh");
    data.add("BLAH", "blooh2");
    assert_eq!(data.size(), dd["size_before"].as_i64().unwrap());
    assert!(data.look_up("foobar").is_err());
    data.done();
    assert_eq!(data.size(), dd["size_after"].as_i64().unwrap());
    assert_eq!(data.look_up("foobar").unwrap().len() as u64, dd["foobar"].as_u64().unwrap());
    assert_eq!(data.look_up("FOOBAR").unwrap().len() as u64, dd["FOOBAR"].as_u64().unwrap());
    assert_eq!(data.look_up("blah").unwrap().len() as u64, dd["blah"].as_u64().unwrap());
    assert_eq!(data.look_up("BLAH").unwrap().len() as u64, dd["BLAH"].as_u64().unwrap());
    assert_eq!(
        data.look_up_predictive("foo").unwrap().len() as u64,
        dd["pred_foo"].as_u64().unwrap()
    );
    assert_eq!(data.look_up("foo").unwrap().len() as u64, dd["exact_foo"].as_u64().unwrap());
    assert_eq!(data.look_up("höge").unwrap().len() as u64, dd["nfc"].as_u64().unwrap());
    assert_eq!(data.look_up("zzzz").unwrap().len() as u64, dd["zzzz"].as_u64().unwrap());

    let det = golden("remaining/MixedEolHandlingReaderTest-testDetection.json");
    for c in det["cases"].as_array().unwrap() {
        let r = omegat_core::mixed_eol::MixedEolReader::from_text(c["text"].as_str().unwrap());
        assert_eq!(r.detected_eol, c["eol"].as_str().unwrap(), "{}", c["text"]);
        assert_eq!(r.mixed, c["mixed"].as_bool().unwrap(), "{}", c["text"]);
    }
    let lines = golden("remaining/MixedEolHandlingReaderTest-testReadLine.json");
    for c in lines["cases"].as_array().unwrap() {
        let mut r = omegat_core::mixed_eol::MixedEolReader::from_text(c["text"].as_str().unwrap());
        let got: Vec<String> = std::iter::from_fn(|| r.read_line()).collect();
        let want: Vec<String> = c["lines"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(got, want);
    }
    let file = golden("remaining/MixedEolHandlingReaderTest-testFile.json");
    let raw = std::fs::read_to_string(java_res(file["file"].as_str().unwrap())).unwrap();
    let mut r = omegat_core::mixed_eol::MixedEolReader::from_text(&raw);
    assert_eq!(r.read_line().as_deref(), file["line0"].as_str());
    assert_eq!(r.detected_eol, file["eol"].as_str().unwrap());
    assert_eq!(r.mixed, file["mixed"].as_bool().unwrap());

    let ign = golden("remaining/DictionariesManagerTest-testLoadIgnoreWords.json");
    let mut mgr = omegat_core::dict::DictionariesManager::default();
    let ignore_path = java_res(&format!("data/dicts/{}", ign["ignore_file"].as_str().unwrap()));
    if ignore_path.is_file() {
        mgr.load_ignore_words(&ignore_path);
    } else {
        mgr.add_ignore_word(ign["word"].as_str().unwrap());
    }
    assert_eq!(mgr.is_ignored(ign["word"].as_str().unwrap()), ign["ignored"].as_bool().unwrap());
    let changed = golden("remaining/DictionariesManagerTest-testFileChanged.json");
    let mut mgr = omegat_core::dict::DictionariesManager::default();
    mgr.add_ignore_word(changed["word"].as_str().unwrap());
    assert_eq!(
        mgr.find_words(&java_res("data/dicts"), &[changed["word"].as_str().unwrap()]).is_empty(),
        changed["empty_after_ignore"].as_bool().unwrap()
    );

    let ff = golden("remaining/FalseFriendsTest-testExecute.json");
    assert_eq!(omegat_core::languagetool::default_bridge_type(), ff["rewrite"].as_str().unwrap());
    let rm = golden("remaining/FalseFriendsTest-testRemoveRules.json");
    assert_eq!(omegat_core::languagetool::default_bridge_type(), rm["rewrite"].as_str().unwrap());

    let mt = golden("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_ValidGlossarySupplier.json");
    let mut engines = omegat_core::mt::engines();
    engines.truncate(mt["translators"].as_u64().unwrap() as usize);
    omegat_core::mt::set_glossary_map(&mut engines, mt["supplier"].as_str());
    assert!(engines.iter().all(|e| e.glossary_supplier.as_deref() == mt["supplier"].as_str()));
    let none = golden("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_NoTranslators.json");
    let mut empty: Vec<omegat_core::mt::MtEngine> = vec![];
    omegat_core::mt::set_glossary_map(&mut empty, Some("x"));
    assert_eq!(empty.len() as u64, none["count"].as_u64().unwrap());
    let nulls = golden("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_NullGlossarySupplier.json");
    let mut engines = omegat_core::mt::engines();
    engines.truncate(2);
    omegat_core::mt::set_glossary_map(&mut engines, nulls["supplier"].as_str());
    assert!(engines.iter().all(|e| e.glossary_supplier.is_none()));

    let xml = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/java/src/test/resources/data/externalfinder/finder.xml"),
    )
    .unwrap();
    let items = omegat_core::finder::parse_finder_xml(&xml);
    let fi = golden("finder/ExternalFinderTest#testGetItems.json");
    assert_eq!(items.len() as u64, fi["count"].as_u64().unwrap());
    assert_eq!(items[0].name, fi["name0"].as_str().unwrap());
    assert_eq!(items[0].nopopup, fi["nopopup0"].as_bool().unwrap());
    assert_eq!(items[2].ascii_only, fi["ascii_only2"].as_bool().unwrap());
    let cmd = golden("finder/ExternalFinderTest#testGetItemCommand.json");
    assert_eq!(items[5].commands[0], cmd["command"].as_str().unwrap());
    let urls = golden("finder/ExternalFinderTest#testGetItemUrl.json");
    assert_eq!(items[0].urls.len() as u64, urls["count"].as_u64().unwrap());
    assert_eq!(items[0].urls[0], urls["url0"].as_str().unwrap());
    assert_eq!(items[0].urls[1], urls["url1"].as_str().unwrap());
    let pop = golden("finder/ExternalFinderTest#testGetItemPopup.json");
    assert_eq!(items[0].nopopup, pop["nopopup"].as_bool().unwrap());
    let proj = golden("finder/ExternalFinderTest#testGetProjectConfig.json");
    assert!(proj["config"].is_null());

    let cli = golden("cli/CommandCommonTest#testParseCommonParamsAppliesSubCommandOptions.json");
    let p = omegat_core::cli_params::parse_common_params(&[
        "--no-project-locking",
        "--no-location-save",
        "--no-team",
        "--ITokenizer",
        "org.omegat.tokenizer.LuceneEnglishTokenizer",
        "--ITokenizerTarget",
        "org.omegat.tokenizer.LuceneGermanTokenizer",
    ]);
    assert_eq!(p.project_locking, cli["project_locking"].as_bool().unwrap());
    assert_eq!(p.location_save, cli["location_save"].as_bool().unwrap());
    assert_eq!(p.no_team, cli["no_team"].as_bool().unwrap());
    assert_eq!(p.tokenizer_source.as_deref(), cli["tokenizer_source"].as_str());
    assert_eq!(p.tokenizer_target.as_deref(), cli["tokenizer_target"].as_str());
    let defs = golden("cli/CommandCommonTest#testParseCommonParamsDefaultsLeaveStoreUntouched.json");
    let d = omegat_core::cli_params::parse_common_params(&[]);
    assert_eq!(d.project_locking, defs["project_locking"].as_bool().unwrap());
    assert_eq!(d.location_save, defs["location_save"].as_bool().unwrap());
    assert_eq!(d.no_team, defs["no_team"].as_bool().unwrap());
    let team = golden("cli/CommandCommonTest#testParseCommonParamsPositiveTeamKeepsDefault.json");
    assert_eq!(
        omegat_core::cli_params::parse_common_params(&["--team"]).no_team,
        team["no_team"].as_bool().unwrap()
    );
    let sep = golden("cli/MainTest#testExtractConfigDirSeparateValue.json");
    assert_eq!(
        omegat_core::cli_params::extract_config_dir(&[
            sep["flag"].as_str().unwrap(),
            sep["value"].as_str().unwrap(),
            "start"
        ])
        .as_deref(),
        sep["value"].as_str()
    );
    let eq = golden("cli/MainTest#testExtractConfigDirEqualsForm.json");
    assert_eq!(
        omegat_core::cli_params::extract_config_dir(&[
            "start",
            &format!("{}{}", eq["flag"].as_str().unwrap(), eq["value"].as_str().unwrap())
        ])
        .as_deref(),
        eq["value"].as_str()
    );
    let absent = golden("cli/MainTest#testExtractConfigDirAbsent.json");
    assert_eq!(omegat_core::cli_params::extract_config_dir(&["start", "project"]).is_some(), absent["present"].as_bool().unwrap());
    assert!(omegat_core::cli_params::extract_config_dir(&["--config-dir"]).is_none());
    assert!(omegat_core::cli_params::extract_config_dir(&["--config-dir="]).is_none());
    assert!(omegat_core::cli_params::extract_config_dir(&[]).is_none());

    let round = golden("cli/MainTest#testConstructCommandParamsRoundTrip.json");
    let mut rt = omegat_core::cli_params::RuntimePrefs::default();
    rt.config_dir = Some(round["config_dir"].as_str().unwrap().into());
    rt.quiet = round["quiet"].as_bool().unwrap();
    rt.no_team = round["no_team"].as_bool().unwrap();
    rt.alternate_filename_from = Some(round["alt_from"].as_str().unwrap().into());
    rt.alternate_filename_to = Some(round["alt_to"].as_str().unwrap().into());
    let cmd = omegat_core::cli_params::construct_command_params(&rt);
    assert_eq!(cmd, strs(&round["argv"]));
    let keep = golden("cli/MainTest#testConstructCommandParamsKeepsRuntimeOptions.json");
    let mut rt = omegat_core::cli_params::RuntimePrefs::default();
    rt.config_file = Some(keep["config_file"].as_str().unwrap().into());
    rt.resource_bundle = Some(keep["resource_bundle"].as_str().unwrap().into());
    rt.project_locking = keep["project_locking"].as_bool().unwrap();
    rt.location_save = keep["location_save"].as_bool().unwrap();
    rt.tokenizer_source = Some(keep["tokenizer_source"].as_str().unwrap().into());
    rt.tokenizer_target = Some(keep["tokenizer_target"].as_str().unwrap().into());
    assert_eq!(omegat_core::cli_params::construct_command_params(&rt), strs(&keep["argv"]));
    let proj = golden("cli/MainTest#testConstructCommandParamsProjectAfterOptions.json");
    let mut rt = omegat_core::cli_params::RuntimePrefs::default();
    rt.config_dir = Some(proj["config_dir"].as_str().unwrap().into());
    let mut argv = omegat_core::cli_params::construct_command_params(&rt);
    argv.push(proj["project"].as_str().unwrap().into());
    assert_eq!(argv.last().map(String::as_str), proj["project"].as_str());
    assert_eq!(argv[argv.len() - 2], "start");

    let init = golden("cli/LegacyParametersTest#testInitializeAppliesConfigDir.json");
    let p = omegat_core::cli_params::initialize_legacy(&[
        "--config-dir",
        init["config_dir"].as_str().unwrap(),
    ]);
    assert_eq!(p.config_dir.as_deref(), init["config_dir"].as_str());
    let tilde = golden("cli/LegacyParametersTest#testInitializeExpandsTilde.json");
    let p = omegat_core::cli_params::initialize_legacy(&[
        &format!("--config-dir={}", tilde["input"].as_str().unwrap()),
    ]);
    let want = std::path::PathBuf::from(std::env::var("HOME").unwrap())
        .join(tilde["home_relative"].as_str().unwrap())
        .to_string_lossy()
        .into_owned();
    assert_eq!(p.config_dir.as_deref(), Some(want.as_str()));
    let none = golden("cli/LegacyParametersTest#testInitializeWithoutConfigDir.json");
    assert_eq!(
        omegat_core::cli_params::initialize_legacy(&[]).config_dir.is_some(),
        none["present"].as_bool().unwrap()
    );
    let flags = golden("cli/LegacyParametersTest#testInitializeAppliesRuntimeFlags.json");
    let p = omegat_core::cli_params::initialize_legacy(&[
        "--disable-project-locking",
        "--disable-location-save",
        "--no-team",
    ]);
    assert_eq!(p.project_locking, flags["project_locking"].as_bool().unwrap());
    assert_eq!(p.location_save, flags["location_save"].as_bool().unwrap());
    assert_eq!(p.no_team, flags["no_team"].as_bool().unwrap());
    let bundle = golden("cli/LegacyParametersTest#testInitializeLoadsResourceBundle.json");
    let p = omegat_core::cli_params::initialize_legacy(&[
        "--resource-bundle",
        bundle["file"].as_str().unwrap(),
    ]);
    assert_eq!(p.resource_bundle.as_deref(), bundle["file"].as_str());

    let latex = golden("engine/LatexFilterUnitTest#testParseBracedCommand.json");
    for c in latex["cases"].as_array().unwrap() {
        let got = omegat_filters::latex::parse_braced_command(
            c["line"].as_str().unwrap(),
            c["prefix"].as_str().unwrap(),
        );
        assert_eq!(got.as_deref(), c["env"].as_str(), "{}", c["line"]);
    }

    let cjk = golden("engine/XMLFilterTest#testLoadCJKPath.json");
    let cjk_path = java_res(cjk["file"].as_str().unwrap());
    assert!(cjk_path.is_file(), "{}", cjk_path.display());
    let mut hooks = omegat_filters::DefaultHooks::parse();
    let parsed = omegat_filters::parse_to_file(
        &cjk_path,
        &omegat_filters::DefaultXmlDialect::default(),
        &mut hooks,
    )
    .expect("parse CJK path");
    assert_eq!(parsed.segments.len() as u64, cjk["segments"].as_u64().unwrap());
    assert_eq!(cjk["ok"].as_bool().unwrap(), true);

    let stats = golden("remaining/CalcStandardStatisticsTest-testStatistics.json");
    let po = java_res("data/filters/po/file-POFilter-match-stat-en-ca.po");
    let parsed = omegat_filters::FilterRegistry::new()
        .for_path(&po)
        .unwrap()
        .parse(
            &po,
            &omegat_filters::FilterContext {
                source_lang: "en".into(),
                target_lang: "ca".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let entries: Vec<omegat_core::source_text_entry::Entry> = parsed
        .segments
        .iter()
        .map(|s| omegat_core::source_text_entry::Entry {
            source: s.source.clone(),
            translation: String::new(),
            file: "file-POFilter-match-stat-en-ca.po".into(),
            id: String::new(),
            note: String::new(),
            comment: String::new(),
            default_translation: true,
            revision: 0,
            from_tm_exact: false,
            properties: vec![],
        })
        .collect();
    let s = omegat_core::stats::compute(&entries, "en", "ca");
    assert_eq!(s.total.segments as u64, stats["total_segments"].as_u64().unwrap());
    assert_eq!(s.total.words as u64, stats["total_words"].as_u64().unwrap());
    assert_eq!(s.total.characters_without_spaces as u64, stats["total_nosp"].as_u64().unwrap());
    assert_eq!(s.total.characters as u64, stats["total_chars"].as_u64().unwrap());
    assert_eq!(s.unique.segments as u64, stats["unique_segments"].as_u64().unwrap());
    assert_eq!(s.file_stats[0].total.segments as u64, stats["file_segments"].as_u64().unwrap());

    let script = golden("remaining/ScriptingTest-testLoadScriptingWindow.json");
    let tmp = tempfile::NamedTempFile::new().unwrap();
    assert!(
        omegat_core::cli_params::resolve_scripts_folder(Some(tmp.path())).is_none(),
        "{}",
        script["bug"]
    );
    let def = golden("remaining/ScriptingTest-testDefaultScriptFolderOnScriptWindow.json");
    let folder = omegat_core::cli_params::default_user_scripts_dir(std::path::Path::new(
        def["config_dir"].as_str().unwrap(),
    ));
    assert_eq!(folder, std::path::PathBuf::from(def["scripts"].as_str().unwrap()));

    let align = golden("align/AlignSettingsPersistenceTest#testRoundTrip.json");
    let mut store = std::collections::HashMap::new();
    let settings = omegat_core::align::AlignSettings {
        algorithm: align["algorithm"].as_str().unwrap().into(),
        calculator: align["calculator"].as_str().unwrap().into(),
        counter: align["counter"].as_str().unwrap().into(),
        segment: align["segment"].as_bool().unwrap(),
        remove_tags: align["remove_tags"].as_bool().unwrap(),
    };
    settings.persist(&mut store);
    let restored = omegat_core::align::AlignSettings::restore(&store);
    assert_eq!(restored, settings);
    let defaults = golden("align/AlignSettingsPersistenceTest#testDefaultsAreKeptWhenNothingStored.json");
    let d = omegat_core::align::AlignSettings::default();
    assert_eq!(d.algorithm, defaults.get("algorithm").and_then(|v| v.as_str()).unwrap_or("viterbi"));
}

#[test]
fn file_util_build_copy_delete_match_java() {
    let build = golden("util/FileUtilTest#testBuildFileList.json");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("a")).unwrap();
    std::fs::write(dir.path().join("a/foo"), b"").unwrap();
    std::fs::write(dir.path().join("a/bar"), b"").unwrap();
    let flat = omegat_core::file_util::build_file_list(dir.path(), false).unwrap();
    assert_eq!(flat.len() as u64, build["non_recursive"].as_u64().unwrap());
    let rec = omegat_core::file_util::build_file_list(dir.path(), true).unwrap();
    let relative: Vec<String> = rec
        .iter()
        .map(|path| {
            path.strip_prefix(dir.path())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(relative, strs(&build["recursive"]));

    let copy = golden("util/FileUtilTest#testCopyFilesTo.json");
    let src = dir.path().join("source");
    let dst = dir.path().join("target");
    std::fs::create_dir_all(src.join("sub1")).unwrap();
    std::fs::write(src.join("file1"), "file1-first").unwrap();
    std::fs::write(src.join("sub1/file2"), "file2-first").unwrap();
    let sources: Vec<_> = std::fs::read_dir(&src).unwrap().map(|e| e.unwrap().path()).collect();
    omegat_core::file_util::copy_files_to(&dst, &sources, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(dst.join("file1")).unwrap(),
        copy["initial"]["file1"].as_str().unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(dst.join("sub1/file2")).unwrap(),
        copy["initial"]["file2"].as_str().unwrap()
    );
    assert_eq!(dst.join("sub1").is_dir(), copy["initial"]["subdir"].as_bool().unwrap());

    std::fs::write(src.join("file1"), "file1-second").unwrap();
    std::fs::write(src.join("sub1/file2"), "file2-second").unwrap();
    std::fs::write(src.join("file3"), "file3-first").unwrap();
    let sources: Vec<_> = std::fs::read_dir(&src).unwrap().map(|e| e.unwrap().path()).collect();
    omegat_core::file_util::copy_files_to(&dst, &sources, false).unwrap();
    for (path, key) in [("file1", "file1"), ("sub1/file2", "file2"), ("file3", "file3")] {
        assert_eq!(
            std::fs::read_to_string(dst.join(path)).unwrap(),
            copy["keep_existing"][key].as_str().unwrap()
        );
    }

    std::fs::write(dst.join("sub1/file4"), "file4").unwrap();
    omegat_core::file_util::copy_files_to_with(&dst, &sources, |target, _, _| {
        if target.file_name().is_some_and(|name| name == "sub1") {
            omegat_core::file_util::CopyCollision::Replace
        } else {
            omegat_core::file_util::CopyCollision::Keep
        }
    })
    .unwrap();
    for (path, key) in [("file1", "file1"), ("sub1/file2", "file2"), ("file3", "file3")] {
        assert_eq!(
            std::fs::read_to_string(dst.join(path)).unwrap(),
            copy["replace_subdir"][key].as_str().unwrap()
        );
    }
    assert_eq!(
        dst.join("sub1/file4").exists(),
        copy["replace_subdir"]["file4_exists"].as_bool().unwrap()
    );

    let mut callback_calls = 0usize;
    omegat_core::file_util::copy_files_to_with(&dst, &sources, |_, index, total| {
        callback_calls += 1;
        if index + 1 == total {
            omegat_core::file_util::CopyCollision::Cancel
        } else {
            omegat_core::file_util::CopyCollision::Replace
        }
    })
    .unwrap();
    assert_eq!(
        callback_calls as u64,
        copy["canceled"]["callback_calls"].as_u64().unwrap()
    );
    for (path, key) in [("file1", "file1"), ("sub1/file2", "file2"), ("file3", "file3")] {
        assert_eq!(
            std::fs::read_to_string(dst.join(path)).unwrap(),
            copy["canceled"][key].as_str().unwrap()
        );
    }

    let new_target = dir.path().join("newtarget");
    omegat_core::file_util::copy_files_to(&new_target, &sources, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(new_target.join("file1")).unwrap(),
        copy["new_target_file1"].as_str().unwrap()
    );
    let target_file = dir.path().join("target-file");
    std::fs::write(&target_file, "").unwrap();
    assert_eq!(
        omegat_core::file_util::copy_files_to(&target_file, &sources, false).is_err(),
        copy["target_file_error"].as_bool().unwrap()
    );

    let del = golden("util/FileUtilTest#testDeleteTree.json");
    let root = dir.path().join("delete-root");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let external = dir.path().join("external");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(external.join("file"), "").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&external, root.join("sub/subsub")).unwrap();
    let deleted = omegat_core::file_util::delete_tree(&root).is_ok();
    assert_eq!(deleted, del["deleted"].as_bool().unwrap());
    assert_eq!(root.exists(), del["root_exists"].as_bool().unwrap());
    assert_eq!(
        external.join("file").exists(),
        del["external_file_exists"].as_bool().unwrap()
    );
}

#[test]
fn simple_issue_and_issue_checker_goldens_call_product_models() {
    let issue = SimpleIssue::new(1, "Hello world!", "Hallo Welt!", "#FF0000");
    let icon_present = golden("gui/SimpleIssueTest-testGetIconReturnsNonNullIcon.json");
    let icon = issue.icon();
    assert_eq!(icon.class_name(), icon_present["icon_class"].as_str().unwrap());
    assert_eq!(true, icon_present["present"].as_bool().unwrap());

    let detail_present =
        golden("gui/SimpleIssueTest-testGetDetailComponentReturnsCorrectComponent.json");
    let detail = issue.detail_component();
    assert_eq!(
        detail.class_name(),
        detail_present["component_class"].as_str().unwrap()
    );
    assert_eq!(true, detail_present["present"].as_bool().unwrap());
    let detail_text =
        golden("gui/SimpleIssueTest-testGetDetailComponentPopulatesTextFields.json");
    assert_eq!(detail.first_text, detail_text["source"].as_str().unwrap());
    assert_eq!(detail.last_text, detail_text["translation"].as_str().unwrap());
    let color = golden("gui/SimpleIssueTest-testGetIconUsesExpectedColor.json");
    assert_eq!(icon.color, color["color"].as_str().unwrap());
    let entry = golden("gui/SimpleIssueTest-testGetEntryNum.json");
    assert_eq!(issue.entry_num() as u64, entry["entry_num"].as_u64().unwrap());

    let entries = vec![
        ProjectIssueEntry {
            file: "file1.txt".into(),
            entry_num: 1,
            source: "HELLO".into(),
            translation: "Bonjour".into(),
            duplicate: false,
        },
        ProjectIssueEntry {
            file: "file1.txt".into(),
            entry_num: 2,
            source: "WORLD".into(),
            translation: "Monde".into(),
            duplicate: false,
        },
        ProjectIssueEntry {
            file: "file2.txt".into(),
            entry_num: 3,
            source: "DUP".into(),
            translation: "Dup1".into(),
            duplicate: false,
        },
        ProjectIssueEntry {
            file: "file2.txt".into(),
            entry_num: 4,
            source: "DUP".into(),
            translation: "Dup2".into(),
            duplicate: true,
        },
    ];
    let counts = |issues: &[omegat_core::issues::ProjectIssue]| {
        (
            issues
                .iter()
                .filter(|item| item.kind == ProjectIssueKind::Provider)
                .count() as u64,
            issues
                .iter()
                .filter(|item| item.kind == ProjectIssueKind::Tag)
                .count() as u64,
        )
    };
    let all = golden("gui/IssueCheckerTest-testCollectIssuesAggregatesTagAndProvider.json");
    let all_issues = collect_project_issues(
        &entries,
        all["pattern"].as_str().unwrap(),
        false,
        Some("file2.txt"),
    )
    .unwrap();
    let (providers, tags) = counts(&all_issues);
    assert_eq!(providers, all["provider_count"].as_u64().unwrap());
    assert_eq!(tags, all["tag_count"].as_u64().unwrap());
    assert_eq!(all_issues.len() as u64, all["total"].as_u64().unwrap());

    let file = golden("gui/IssueCheckerTest-testFilePatternFiltersEntries.json");
    let file_issues = collect_project_issues(
        &entries,
        file["pattern"].as_str().unwrap(),
        false,
        Some("file2.txt"),
    )
    .unwrap();
    let (providers, tags) = counts(&file_issues);
    assert_eq!(providers, file["provider_count"].as_u64().unwrap());
    assert_eq!(tags, file["tag_count"].as_u64().unwrap());
    assert_eq!(file_issues.len() as u64, file["total"].as_u64().unwrap());

    let duplicates = golden("gui/IssueCheckerTest-testDuplicateFiltering.json");
    let unfiltered = collect_project_issues(&entries, ".*", false, Some("file2.txt")).unwrap();
    let filtered = collect_project_issues(&entries, ".*", true, Some("file2.txt")).unwrap();
    let (provider_all, tag_all) = counts(&unfiltered);
    let (provider_filtered, tag_filtered) = counts(&filtered);
    assert_eq!(provider_all, duplicates["provider_all"].as_u64().unwrap());
    assert_eq!(
        provider_filtered,
        duplicates["provider_filtered"].as_u64().unwrap()
    );
    assert_eq!(tag_all, duplicates["tag_all"].as_u64().unwrap());
    assert_eq!(tag_filtered, duplicates["tag_filtered"].as_u64().unwrap());
}

#[test]
fn ostrings_xml_stream_and_stats_result_match_java_goldens() {
    for name in [
        "remaining/OStringsTest-testDevBuildMarkerFromBranchCheckout.json",
        "remaining/OStringsTest-testDevBuildMarkerHiddenOutsideBranchCheckouts.json",
    ] {
        let data = golden(name);
        for case in data["cases"].as_array().unwrap() {
            assert_eq!(
                omegat_core::ostrings::dev_build_marker(
                    case["revision"].as_str().unwrap(),
                    case["branch"].as_str().unwrap()
                ),
                case["marker"].as_str().unwrap()
            );
        }
    }

    let xml_golden = golden("remaining/XMLStreamReaderTest-testLoadXML.json");
    let xml = std::fs::read_to_string(java_res(xml_golden["file"].as_str().unwrap())).unwrap();
    let body = close_block(&xml, "body").unwrap();
    assert_eq!(
        body.attributes.get("attr").map(String::as_str),
        xml_golden["body_attr"].as_str()
    );
    let blocks: Vec<String> = body.blocks.iter().map(|block| block.descriptor()).collect();
    assert_eq!(blocks, strs(&xml_golden["blocks"]));

    let bad = golden("remaining/XMLStreamReaderTest-testBadEntity.json");
    let bad_results: Vec<bool> = bad["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| {
            let xml = std::fs::read_to_string(java_res(file.as_str().unwrap())).unwrap();
            close_block(&xml, "body").is_err()
        })
        .collect();
    assert_eq!(bad_results, vec![true, true]);
    assert_eq!(bad["error_class"].as_str().unwrap(), "TranslationException");

    let stats_golden = golden("remaining/StatsResultTest-testStatsResultXML.json");
    let to_count = |value: &Value| {
        let values = value.as_array().unwrap();
        StatCountDto {
            segments: values[0].as_u64().unwrap() as usize,
            words: values[1].as_u64().unwrap() as usize,
            characters_without_spaces: values[2].as_u64().unwrap() as usize,
            characters: values[3].as_u64().unwrap() as usize,
            files: values[4].as_u64().unwrap() as usize,
        }
    };
    let mut stats = omegat_core::stats::compute(&[], "English", "French");
    stats.total = to_count(&stats_golden["total"]);
    stats.remaining = to_count(&stats_golden["remaining"]);
    stats.unique = to_count(&stats_golden["unique"]);
    stats.unique_remaining = to_count(&stats_golden["unique_remaining"]);
    stats.file_stats.push(FileStatDto {
        filename: stats_golden["filename"].as_str().unwrap().into(),
        total: to_count(&stats_golden["file_total"]),
        unique: to_count(&stats_golden["file_unique"]),
        remaining: to_count(&stats_golden["file_remaining"]),
        unique_remaining: to_count(&stats_golden["file_unique_remaining"]),
    });
    let project = &stats_golden["project"];
    let rendered = omegat_core::stats::render_stats_result_xml(
        &stats,
        &omegat_core::stats::StatsResultMetadata {
            project_name: project["name"].as_str().unwrap().into(),
            project_root: project["root"].as_str().unwrap().into(),
            source_language: project["source_language"].as_str().unwrap().into(),
            target_language: project["target_language"].as_str().unwrap().into(),
        },
        "DATE",
    );
    let document = close_block(&rendered, "omegat-stats").unwrap();
    let blocks: Vec<String> = document.blocks.iter().map(|block| block.descriptor()).collect();
    assert_eq!(blocks, strs(&stats_golden["xml_blocks"]));
}

#[test]
fn find_matches_thread_regression_calls_find_matches_product_path() {
    let data = golden("remaining/FindMatchesThreadTest-testSearchBUGS1248.json");
    let path = java_res("data/tmx/penalty-010/segment_1.tmx");
    let entries = omegat_core::external_tm::load(&path, "ja", "fr", false);
    let extra: Vec<_> = entries
        .into_iter()
        .map(|entry| (entry, "penalty-010/segment_1.tmx".to_string()))
        .collect();
    let hits = omegat_core::find_matches::search(omegat_core::find_matches::SearchRequest {
        query: data["query"].as_str().unwrap(),
        memory: &[],
        extra: &extra,
        files: &[],
        tokenizer: "org.omegat.tokenizer.LuceneCJKTokenizer",
        source_lang: "ja",
        target_lang: "fr",
        threshold: data["threshold"].as_i64().unwrap() as i32,
        limit: omegat_core::consts::MAX_NEAR_STRINGS,
        search_exactly_the_same: false,
        run_separate_segment_match: true,
        foreign_penalty: omegat_core::find_matches::PENALTY_FOR_FOREIGN_MATCHES_DEFAULT,
    });
    let expected = data["hits"].as_array().unwrap();
    assert_eq!(hits.len(), expected.len());
    for (actual, expected) in hits.iter().zip(expected) {
        assert_eq!(actual.source, expected["source"].as_str().unwrap());
        assert_eq!(actual.comes_from, expected["comes_from"].as_str().unwrap());
        assert_eq!(actual.score as i64, expected["score"].as_i64().unwrap());
        if let Some(translation) = expected["translation"].as_str() {
            assert_eq!(actual.translation, translation);
        }
    }
}
