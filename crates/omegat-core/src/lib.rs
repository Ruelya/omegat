//! OmegaT core engine: projects, TMX, segmentation, matching, compile.

pub mod align;
pub mod consts;
pub mod dict;
pub mod error;
pub mod finder;
pub mod glossary;
pub mod languagetool;
pub mod matching;
pub mod mt;
pub mod prefs;
pub mod properties;
pub mod search;
pub mod segment;
pub mod session;
pub mod spell;
pub mod stats;
pub mod tags;
pub mod tmx;
pub mod tokenize;
pub mod wiki;

pub use error::{CoreError, Result};
pub use prefs::Preferences;
pub use properties::ProjectProperties;
pub use session::{Entry, ProjectSession};

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
        std::fs::write(session.props.source_dir.join("a.txt"), "Hello world.\n\nSecond.").unwrap();
        drop(session);
        session = ProjectSession::open(&root, prefs).unwrap();
        assert!(session.entries.len() >= 2);
        let rev = session.entries[0].revision;
        session
            .set_entry(&omegat_ipc::SetEntryParams {
                index: 0,
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
                translation: "x".into(),
                note: None,
                revision: 999,
                default_translation: true,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::OptimisticLock(0)));
    }

    #[test]
    fn tag_validation_abort_on_set() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("proj3");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs.extra.insert("tag_validation".into(), "abort".into());
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
        std::fs::write(session.props.source_dir.join("a.txt"), "Hello world\n\nOther").unwrap();
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
        let hello = session.entries.iter().find(|e| e.source == "Hello world").unwrap();
        assert_eq!(hello.translation, "AUTO_HIT");
        let idx = session.entries.iter().position(|e| e.source == "Hello world").unwrap();
        let hits = session.matches_for(idx);
        assert!(hits.iter().any(|h| h.translation == "MT_HIT"));
        assert!(hits.iter().any(|h| h.translation == "PENALTY_HIT" && h.score <= 90));

        write_tm("enforce", "ENFORCE_HIT");
        drop(session);
        session = ProjectSession::open(&root, prefs).unwrap();
        let hello = session.entries.iter().find(|e| e.source == "Hello world").unwrap();
        assert_eq!(hello.translation, "ENFORCE_HIT");
    }

    #[test]
    fn compile_text_html_po_java_goldens() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("goldproj");
        let mut prefs = Preferences::default_in(dir.path().join("cfg"));
        prefs.extra.insert("skipHeader".into(), "true".into());
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
        std::fs::copy(fixtures.join("text/file-TextFilter.txt"), session.props.source_dir.join("a.txt")).unwrap();
        std::fs::copy(fixtures.join("html/file-HTMLFilter2.html"), session.props.source_dir.join("b.html")).unwrap();
        std::fs::copy(fixtures.join("po/file-POFilter-multiple.po"), session.props.source_dir.join("c.po")).unwrap();
        session.reload().unwrap();
        assert!(session.entries.iter().any(|e| e.source == "This test file for test TextFilter."));
        assert!(session.entries.iter().any(|e| e.source == "This is first line."));
        assert!(session.entries.iter().any(|e| e.source == "source3"));
        for e in &mut session.entries {
            if e.source == "This test file for test TextFilter." {
                e.translation = "GOLDEN_T".into();
            }
            if e.source == "This is first line." {
                e.translation = "Ceci est la premiere ligne.".into();
            }
            if e.source == "source3" {
                e.translation = "GOLDEN_T".into();
            }
        }
        session.compile(None).unwrap();
        let txt = std::fs::read_to_string(session.props.target_dir.join("a.txt")).unwrap();
        let html = std::fs::read_to_string(session.props.target_dir.join("b.html")).unwrap();
        let po = std::fs::read_to_string(session.props.target_dir.join("c.po")).unwrap();
        assert!(txt.contains("GOLDEN_T"), "{txt}");
        assert!(html.contains("Ceci est la premiere ligne."), "{html}");
        assert!(po.contains("GOLDEN_T"), "{po}");
        let omegat = session.tmx.to_xml_level("en", "fr", "omegat");
        let back = crate::tmx::parse_tmx(&omegat, "en", "fr");
        assert!(!back.entries.is_empty());
    }
}
