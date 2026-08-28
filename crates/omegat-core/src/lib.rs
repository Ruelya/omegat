//! OmegaT core engine: projects, TMX, segmentation, matching, compile.

pub mod align;
pub mod bidi;
pub mod cancellation;
pub mod cli_params;
pub mod completer;
pub mod consts;
pub mod dict;
pub mod durable_file;
pub mod encoding;
pub mod entity_util;
pub mod error;
pub mod external_tm;
pub mod file_progress;
pub mod file_util;
pub mod find_matches;
pub mod finder;
pub mod glossary;
pub mod http_url;
pub mod import;
pub mod issues;
pub mod json_parser;
pub mod known_exception;
pub mod language;
pub mod languagetool;
pub mod last_segment;
pub mod levenshtein;
pub mod magic_comment;
pub mod matches_text;
pub mod matches_var;
pub mod matching;
pub mod mixed_eol;
pub mod mt;
pub mod ostrings;
pub mod pattern_consts;
pub mod prefs;
pub mod properties;
pub mod real_project;
pub mod search;
pub mod segment;
pub mod segmented_history;
pub mod session;
pub mod source_text_entry;
pub mod spell;
pub mod srx;
pub mod static_utils;
pub mod stats;
pub mod string_util;
pub mod tag_repair;
pub mod tag_validation;
pub mod tags;
pub mod tmx;
pub mod tokenize;
pub mod version;
pub mod wiki;
pub mod xml_stream;

pub use error::{CoreError, Result};
pub use prefs::Preferences;
pub use properties::ProjectProperties;
pub use session::ProjectSession;
pub use source_text_entry::Entry;

use omegat_ipc::{Capabilities, VersionInfo};

pub fn version() -> VersionInfo {
    VersionInfo::default()
}

pub fn capabilities() -> Capabilities {
    ProjectSession::capabilities()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omegat_ipc::CreateProjectParams;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn create_open_translate_compile() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("proj");
        let prefs = Preferences::default_in(dir.path().join("cfg"));
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs.clone(),
        )
        .unwrap();
        std::fs::write(
            session.props.source_dir.join("a.txt"),
            "Hello world.\n\nSecond.",
        )
        .unwrap();
        drop(session);
        session = ProjectSession::open(&root, prefs).unwrap();
        assert!(session.entries.len() >= 2);
        let rev = session.entries[0].revision;
        session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: None,
                translation: "Bonjour le monde.".into(),
                note: None,
                revision: rev,
                default_translation: true,
            })
            .unwrap();
        session.save().unwrap();
        assert!(session.props.save_tmx_path().exists());
        let n = session.compile(None).unwrap();
        assert!(n >= 1);
        let target = std::fs::read_to_string(session.props.target_dir.join("a.txt")).unwrap();
        assert!(target.contains("Bonjour") || target.contains("Bonjour le monde"));
    }

    #[test]
    fn optimistic_lock() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("proj2");
        let prefs = Preferences::default_in(dir.path().join("cfg"));
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "de".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        std::fs::write(session.props.source_dir.join("a.txt"), "Hi").unwrap();
        session.reload().unwrap();
        let err = session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: None,
                translation: "x".into(),
                note: None,
                revision: 999,
                default_translation: true,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::OptimisticLock(0)));
    }

    #[test]
    fn default_translation_propagates_and_alternative_is_occurrence_scoped() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("translation-kinds");
        let prefs = Preferences::default_in(dir.path().join("cfg"));
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        session.entries = vec![
            Entry {
                file: "a.txt".into(),
                id: "first".into(),
                prev: Some(String::new()),
                next: Some("Repeated".into()),
                path: Some("/first".into()),
                source: "Repeated".into(),
                translation: "old default".into(),
                note: String::new(),
                comment: String::new(),
                default_translation: true,
                revision: 1,
                from_tm_exact: false,
                properties: vec![],
            },
            Entry {
                file: "a.txt".into(),
                id: "second".into(),
                prev: Some("Repeated".into()),
                next: Some(String::new()),
                path: Some("/second".into()),
                source: "Repeated".into(),
                translation: "old default".into(),
                note: String::new(),
                comment: String::new(),
                default_translation: true,
                revision: 1,
                from_tm_exact: false,
                properties: vec![],
            },
            Entry {
                file: "b.txt".into(),
                id: "third".into(),
                prev: Some(String::new()),
                next: Some(String::new()),
                path: Some("/third".into()),
                source: "Repeated".into(),
                translation: "existing alternative".into(),
                note: "private note".into(),
                comment: String::new(),
                default_translation: false,
                revision: 1,
                from_tm_exact: false,
                properties: vec![],
            },
        ];
        session
            .tmx
            .set_default_translation("Repeated", "old default");
        session.tmx.set_occurrence_translation_for_key(
            &session.entries[2].key(),
            "existing alternative",
            Some("private note".into()),
        );

        let propagated = session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: None,
                translation: "shared".into(),
                note: Some("shared note".into()),
                revision: 1,
                default_translation: true,
            })
            .unwrap();
        assert_eq!(
            propagated
                .updated
                .iter()
                .map(|entry| (
                    entry.index,
                    entry.translation.as_str(),
                    entry.note.as_str(),
                    entry.default_translation,
                    entry.revision,
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, "shared", "shared note", true, 2),
                (1, "shared", "shared note", true, 2),
            ]
        );
        assert_eq!(
            session
                .entries
                .iter()
                .map(|entry| (
                    entry.translation.as_str(),
                    entry.note.as_str(),
                    entry.default_translation,
                    entry.revision,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("shared", "shared note", true, 2),
                ("shared", "shared note", true, 2),
                ("existing alternative", "private note", false, 1),
            ]
        );

        let alternative = session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 1,
                key: None,
                translation: "second only".into(),
                note: Some("second note".into()),
                revision: 2,
                default_translation: false,
            })
            .unwrap();
        assert_eq!(
            alternative
                .updated
                .iter()
                .map(|entry| (
                    entry.index,
                    entry.translation.as_str(),
                    entry.default_translation
                ))
                .collect::<Vec<_>>(),
            vec![(1, "second only", false)]
        );
        assert_eq!(
            session
                .entries
                .iter()
                .map(|entry| (entry.translation.as_str(), entry.default_translation))
                .collect::<Vec<_>>(),
            vec![
                ("shared", true),
                ("second only", false),
                ("existing alternative", false),
            ]
        );

        let back_to_default = session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 1,
                key: None,
                translation: "new shared".into(),
                note: Some("new shared note".into()),
                revision: 3,
                default_translation: true,
            })
            .unwrap();
        assert_eq!(
            back_to_default
                .updated
                .iter()
                .map(|entry| (
                    entry.index,
                    entry.translation.as_str(),
                    entry.default_translation
                ))
                .collect::<Vec<_>>(),
            vec![(0, "new shared", true), (1, "new shared", true)]
        );
        assert_eq!(
            session
                .tmx
                .get_translation_for_key(&session.entries[1].key())
                .map(|entry| (entry.translation.as_str(), entry.default_translation)),
            Some(("new shared", true))
        );
        assert_eq!(
            session
                .tmx
                .get_translation_for_key(&session.entries[2].key())
                .map(|entry| (entry.translation.as_str(), entry.default_translation)),
            Some(("existing alternative", false))
        );
    }

    #[test]
    fn alternatives_with_same_file_id_and_source_persist_by_complete_entry_key() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("complete-entry-keys");
        let prefs = Preferences::default_in(dir.path().join("cfg"));
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        session.entries = vec![
            Entry {
                file: "same.po".into(),
                id: "message".into(),
                prev: Some(String::new()),
                next: Some("Repeated".into()),
                path: Some("dialog/one".into()),
                source: "Repeated".into(),
                translation: String::new(),
                note: String::new(),
                comment: String::new(),
                default_translation: true,
                revision: 1,
                from_tm_exact: false,
                properties: vec![],
            },
            Entry {
                file: "same.po".into(),
                id: "message".into(),
                prev: Some("Repeated".into()),
                next: Some(String::new()),
                path: Some("dialog/two".into()),
                source: "Repeated".into(),
                translation: String::new(),
                note: String::new(),
                comment: String::new(),
                default_translation: true,
                revision: 1,
                from_tm_exact: false,
                properties: vec![],
            },
        ];
        let first = session.entries[0].key();
        let second = session.entries[1].key();
        let mut changed_key = first.clone();
        changed_key.path = Some("dialog/reloaded".into());
        assert!(matches!(
            session.set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: Some(changed_key),
                translation: "wrong occurrence".into(),
                note: None,
                revision: 1,
                default_translation: false,
            }),
            Err(CoreError::InvalidProject(_))
        ));

        session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: Some(first.clone()),
                translation: "Premier".into(),
                note: Some("first note".into()),
                revision: 1,
                default_translation: false,
            })
            .unwrap();
        session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 1,
                key: Some(second.clone()),
                translation: "Deuxième".into(),
                note: Some("second note".into()),
                revision: 1,
                default_translation: false,
            })
            .unwrap();
        session.save().unwrap();

        let loaded = crate::tmx::ProjectTmx::load(
            &session.props.save_tmx_path(),
            &session.props.source_lang,
            &session.props.target_lang,
        )
        .unwrap();
        assert_eq!(
            (
                loaded
                    .get_translation_for_key(&first)
                    .map(|entry| (
                        entry.translation.as_str(),
                        entry.note.as_deref(),
                        entry.prev.as_deref(),
                        entry.next.as_deref(),
                        entry.path.as_deref(),
                    )),
                loaded
                    .get_translation_for_key(&second)
                    .map(|entry| (
                        entry.translation.as_str(),
                        entry.note.as_deref(),
                        entry.prev.as_deref(),
                        entry.next.as_deref(),
                        entry.path.as_deref(),
                    )),
            ),
            (
                Some((
                    "Premier",
                    Some("first note"),
                    Some(""),
                    Some("Repeated"),
                    Some("dialog/one"),
                )),
                Some((
                    "Deuxième",
                    Some("second note"),
                    Some("Repeated"),
                    Some(""),
                    Some("dialog/two"),
                )),
            )
        );
    }

    #[test]
    fn tag_validation_abort_on_set() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("proj3");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs.tag_validation = "abort".into();
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        std::fs::write(session.props.source_dir.join("a.txt"), "Hello <b>x</b>").unwrap();
        session.reload().unwrap();
        let rev = session.entries[0].revision;
        let err = session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: None,
                translation: "Bonjour x".into(),
                note: None,
                revision: rev,
                default_translation: true,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::TagValidation(_)));
    }

    #[test]
    fn tm_folders_auto_enforce_mt_penalty() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("tmfold");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs
            .filter_context
            .insert("segmentOn".into(), "BREAKS".into());
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs.clone(),
        )
        .unwrap();
        std::fs::write(
            session.props.source_dir.join("a.txt"),
            "Hello world\n\nOther",
        )
        .unwrap();
        let tm_dir = session.props.tm_dir.clone();
        let write_tm = |folder: &str, trans: &str| {
            let tmx = format!(
                r#"<?xml version="1.0"?><tmx version="1.4"><body>
                <tu><tuv xml:lang="en"><seg>Hello world</seg></tuv>
                <tuv xml:lang="fr"><seg>{trans}</seg></tuv></tu></body></tmx>"#
            );
            let p = tm_dir.join(folder);
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("mem.tmx"), tmx).unwrap();
        };
        write_tm("auto", "AUTO_HIT");
        write_tm("mt", "MT_HIT");
        write_tm("penalty-010", "PENALTY_HIT");
        drop(session);
        session = ProjectSession::open(&root, prefs.clone()).unwrap();
        let hello = session
            .entries
            .iter()
            .find(|e| e.source == "Hello world")
            .unwrap();
        assert_eq!(hello.translation, "AUTO_HIT");
        let idx = session
            .entries
            .iter()
            .position(|e| e.source == "Hello world")
            .unwrap();
        let hits = session.matches_for(idx);
        assert!(hits.iter().any(|h| h.translation == "MT_HIT"));
        assert!(hits
            .iter()
            .any(|h| h.translation == "PENALTY_HIT" && h.score <= 90));

        write_tm("enforce", "ENFORCE_HIT");
        drop(session);
        session = ProjectSession::open(&root, prefs).unwrap();
        let hello = session
            .entries
            .iter()
            .find(|e| e.source == "Hello world")
            .unwrap();
        assert_eq!(hello.translation, "ENFORCE_HIT");
    }

    fn load_golden(rel: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/goldens")
            .join(rel);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn normalize_ws(s: &str) -> String {
        s.replace("\r\n", "\n").replace('\r', "\n")
    }

    #[test]
    fn compile_text_html_po_java_goldens() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("goldproj");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs
            .filter_context
            .insert("skipHeader".into(), "true".into());
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/filters");
        std::fs::copy(
            fixtures.join("text/file-TextFilter.txt"),
            session.props.source_dir.join("a.txt"),
        )
        .unwrap();
        std::fs::copy(
            fixtures.join("html/file-HTMLFilter2.html"),
            session.props.source_dir.join("b.html"),
        )
        .unwrap();
        std::fs::copy(
            fixtures.join("po/file-POFilter-multiple.po"),
            session.props.source_dir.join("c.po"),
        )
        .unwrap();
        session.reload().unwrap();
        let text_g = load_golden("filters/text/file-TextFilter.empty-lines.json");
        let html_g = load_golden("filters/html/file-HTMLFilter2.json");
        let po_g = load_golden("filters/po/file-POFilter-multiple.json");
        for e in &mut session.entries {
            e.translation.clear();
        }
        for g in [&text_g, &html_g, &po_g] {
            let src = g["translated"]["source"].as_str().unwrap();
            let tr = g["translated"]["translation"].as_str().unwrap();
            let mut found = false;
            for e in &mut session.entries {
                if e.source == src {
                    e.translation = tr.to_string();
                    found = true;
                }
            }
            assert!(found, "missing session entry for {:?}", src);
        }
        session.compile(None).unwrap();
        let txt = std::fs::read_to_string(session.props.target_dir.join("a.txt")).unwrap();
        let html = std::fs::read_to_string(session.props.target_dir.join("b.html")).unwrap();
        let po = std::fs::read_to_string(session.props.target_dir.join("c.po")).unwrap();
        assert_eq!(
            normalize_ws(&txt),
            normalize_ws(text_g["translated_write"].as_str().unwrap())
        );
        assert_eq!(
            normalize_ws(&html),
            normalize_ws(html_g["translated_write"].as_str().unwrap())
        );
        assert_eq!(
            normalize_ws(&po),
            normalize_ws(po_g["translated_write"].as_str().unwrap())
        );
        let omegat = session.tmx.to_xml_level("en", "fr", "omegat");
        let back = crate::tmx::parse_tmx(&omegat, "en", "fr");
        assert_eq!(back.entries.len(), session.tmx.entries.len());
    }

    #[test]
    fn java_sample_project_tmx_roundtrip() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/java/src/testAcceptance/resources/data/project");
        let dir = tempdir().unwrap();
        let root = dir.path().join("sample");
        copy_tree(&src, &root);
        let prefs = Preferences::default_in(dir.path().join("cfg"));
        let mut session = ProjectSession::open(&root, prefs.clone()).unwrap();
        let n = session.entries.len();
        assert!(n > 0, "sample project produced no segments");
        session.save().unwrap();
        let tmx_path = session.props.save_tmx_path();
        let saved = crate::tmx::ProjectTmx::load(&tmx_path, "en", "fr").unwrap();
        drop(session);
        let session2 = ProjectSession::open(&root, prefs).unwrap();
        assert_eq!(session2.entries.len(), n);
        for e in &session2.entries {
            if e.translated() {
                let hit = saved
                    .get(&e.source)
                    .unwrap_or_else(|| panic!("tmx missing {}", e.source));
                assert_eq!(hit.translation, e.translation);
            }
        }
    }

    #[test]
    fn en_de_fuzzy_top1_same_entry() {
        let mem = vec![
            crate::tmx::TmxEntry {
                source: "Hello world".into(),
                translation: "Bonjour le monde".into(),
                ..Default::default()
            },
            crate::tmx::TmxEntry {
                source: "Hallo Welt".into(),
                translation: "Hello world".into(),
                ..Default::default()
            },
        ];
        let q = "Hello word";
        let en = crate::matching::find_matches(q, &mem, &[], "en");
        let de = crate::matching::find_matches(q, &mem, &[], "de");
        assert!(!en.is_empty() && !de.is_empty());
        assert_eq!(en[0].source, de[0].source);
        assert_eq!(
            en[0].score, de[0].score,
            "en/de score delta must be recorded; got en={} de={}",
            en[0].score, de[0].score
        );
    }

    #[test]
    fn tag_validation_warn_does_not_abort() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("projw");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs.tag_validation = "warn".into();
        let mut session = ProjectSession::create(
            &CreateProjectParams {
                root: root.to_string_lossy().into(),
                source_lang: "en".into(),
                target_lang: "fr".into(),
                sentence_seg: false,
            },
            prefs,
        )
        .unwrap();
        std::fs::write(session.props.source_dir.join("a.txt"), "Hello <b>x</b>").unwrap();
        session.reload().unwrap();
        let rev = session.entries[0].revision;
        session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
                key: None,
                translation: "Bonjour x".into(),
                note: None,
                revision: rev,
                default_translation: true,
            })
            .unwrap();
        let issues = session.issues();
        assert!(
            issues
                .iter()
                .any(|i| i.kind == "tag" && i.message.contains("MISSING")),
            "{issues:?}"
        );
        let cancellation = crate::cancellation::CancellationToken::default();
        cancellation.cancel();
        assert!(session.issues_cancellable(&cancellation).is_none());
    }

    fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
        for ent in walkdir::WalkDir::new(src).into_iter().flatten() {
            let rel = ent.path().strip_prefix(src).unwrap();
            let name = rel.to_string_lossy().replace('\\', "/");
            if name.contains("/target/") || name.starts_with("target/") || name.ends_with(".lock") {
                continue;
            }
            let dest = dst.join(rel);
            if ent.file_type().is_dir() {
                std::fs::create_dir_all(&dest).ok();
            } else {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p).ok();
                }
                std::fs::copy(ent.path(), dest).unwrap();
            }
        }
    }
}
