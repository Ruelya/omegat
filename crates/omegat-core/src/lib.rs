//! OmegaT core engine: projects, TMX, segmentation, matching, compile.

pub mod align;
pub mod consts;
pub mod dict;
pub mod error;
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
}
