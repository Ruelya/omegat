//! Team project sync aligned with Java `team2` (23 classes, one Rust file each).
//!
//! Layout: `.repositories/<sanitized-url>/` is the remote working copy;
//! `.repositories/prep/` holds the last-synced TMX/glossary base and conflict list.
//! `sync` is prepare → rebase (TMX **and** glossary) → commit/push.

mod error;
mod file_repository;
mod git2_ops;
mod git_credentials_provider;
mod git_remote_repository2;
mod glossary_rebase;
mod http_remote_repository;
mod i_rebase_operation;
mod i_remote_repository2;
mod mapping;
mod passphrase_dialog;
mod prepared_file_info;
mod project_team_settings;
mod rebase_and_commit;
mod rebase_utils;
mod remote_repository_factory;
mod remote_repository_provider;
mod repositories_credentials_controller;
mod repositories_credentials_panel;
mod svn_authentication_manager;
mod svn_remote_repository2;
mod team_settings;
mod team_tool;
mod team_utils;
mod tmx_rebase;
mod transaction_envelope;
mod user_pass_dialog;

pub use error::{Conflict, SyncReport, TeamError};
pub use mapping::{
    copy_mapped, copy_mapped_from_worktree, default_mapping, glob_match, propagate_deleted, CopyDir,
};
pub use passphrase_dialog::Passphrase;
pub use prepared_file_info::PreparedFileInfo;
pub use project_team_settings::{REPO_PREP, REPO_SUBDIR};
pub use rebase_and_commit::{
    rebase_all, rebase_project, resolve, resolve_for_key, resolve_for_key_cancellable,
    resolve_for_key_cancellable_scoped,
};
pub use remote_repository_factory::detect_repository_type;
pub use remote_repository_provider::{
    acknowledge_renderer_receipt, commit_after_version, commit_product_transaction_cancellable,
    commit_project_files, commit_project_files_cancellable,
    commit_project_files_cancellable_scoped, get_version, pending_renderer_receipt,
    recover_interrupted_sync, switch_to_version, sync, sync_cancellable, sync_cancellable_scoped,
    TeamRendererAck, TeamRendererReceipt,
};
pub use repositories_credentials_panel::{CredentialsPanel, RepositoryCredentials};
pub use team_settings::list_conflicts;
pub use team_tool::init;
pub use team_utils::{
    relative_remote_to_absolute_local, with_leading_slash, with_slashes, without_slashes,
};
pub use tmx_rebase::rebase_tmx;
pub use transaction_envelope::{
    write_json_atomic, TransactionCommit, TransactionEnvelope, TransactionStatus,
    REQUEST_CANCELLED_CODE, TRANSACTION_ENVELOPE_VERSION,
};
pub use user_pass_dialog::UserPass;

pub type Result<T> = error::Result<T>;

pub fn team_enabled() -> bool {
    std::env::var("OMEGAT_NO_TEAM").ok().as_deref() != Some("1")
}

/// Java `org.omegat.core.team2` compilation units that have a matching Rust file.
pub const TEAM2_JAVA_CLASSES: &[&str] = &[
    "IRemoteRepository2",
    "PreparedFileInfo",
    "RebaseAndCommit",
    "TeamSettings",
    "UserPassDialog",
    "PassphraseDialog",
    "RepositoriesCredentialsController",
    "RepositoriesCredentialsPanel",
    "TeamTool",
    "RemoteRepositoryProvider",
    "GlossaryRebaseOperation",
    "TMXRebaseOperation",
    "RebaseUtils",
    "IRebaseOperation",
    "ProjectTeamSettings",
    "RemoteRepositoryFactory",
    "TeamUtils",
    "GITCredentialsProvider",
    "SVNRemoteRepository2",
    "HTTPRemoteRepository",
    "GITRemoteRepository2",
    "SVNAuthenticationManager",
    "FileRepository",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::default_mapping;
    use crate::team_utils::{run_cmd, which};
    use omegat_core::properties::{ProjectProperties, RepositoryDef, RepositoryMapping};
    use omegat_core::tmx::{parse_tmx, ProjectTmx, TmxEntry};
    use omegat_ipc::EntryKeyDto;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    fn tu(src: &str, tgt: &str) -> String {
        format!(
            r#"<tu><tuv lang="en"><seg>{src}</seg></tuv><tuv lang="fr"><seg>{tgt}</seg></tuv></tu>"#
        )
    }

    fn team_props(
        root: PathBuf,
        repo_type: &str,
        url: &str,
        mappings: Vec<RepositoryMapping>,
    ) -> ProjectProperties {
        let mut props = ProjectProperties::create(root, "en".into(), "fr".into(), false);
        props.repositories.push(RepositoryDef {
            repo_type: repo_type.into(),
            url: url.into(),
            branch: Some("main".into()),
            mappings,
        });
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        props
    }

    fn write_tmx(path: &Path, pairs: &[(&str, &str)]) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut raw = String::new();
        for (s, t) in pairs {
            raw.push_str(&tu(s, t));
        }
        std::fs::write(path, raw).unwrap();
    }

    #[test]
    fn factory_detects_url_prefixes() {
        assert_eq!(
            detect_repository_type("svn://example.com/repo"),
            Some("svn")
        );
        assert_eq!(
            detect_repository_type("git://example.com/repo"),
            Some("git")
        );
        assert_eq!(
            detect_repository_type("https://git.example.com/repo"),
            Some("git")
        );
        assert_eq!(
            detect_repository_type("https://example.com/repo.git"),
            Some("git")
        );
    }

    #[test]
    fn team2_has_one_module_per_java_class() {
        assert_eq!(TEAM2_JAVA_CLASSES.len(), 23);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for name in TEAM2_JAVA_CLASSES {
            let snake = java_to_snake(name);
            assert!(
                root.join(format!("{snake}.rs")).exists(),
                "missing Rust module for {name} ({snake}.rs)"
            );
        }
    }

    fn java_to_snake(name: &str) -> String {
        match name {
            "IRemoteRepository2" => "i_remote_repository2".into(),
            "IRebaseOperation" => "i_rebase_operation".into(),
            "GITRemoteRepository2" => "git_remote_repository2".into(),
            "GITCredentialsProvider" => "git_credentials_provider".into(),
            "SVNRemoteRepository2" => "svn_remote_repository2".into(),
            "SVNAuthenticationManager" => "svn_authentication_manager".into(),
            "HTTPRemoteRepository" => "http_remote_repository".into(),
            "TMXRebaseOperation" => "tmx_rebase".into(),
            "GlossaryRebaseOperation" => "glossary_rebase".into(),
            other => {
                let mut out = String::new();
                for (i, c) in other.chars().enumerate() {
                    if c.is_uppercase() {
                        if i > 0 {
                            out.push('_');
                        }
                        out.extend(c.to_lowercase());
                    } else {
                        out.push(c);
                    }
                }
                out
            }
        }
    }

    #[test]
    fn rebase_keeps_ours_and_flags_conflict() {
        let ours = tu("Hi", "Salut");
        let theirs = tu("Hi", "Bonjour");
        let (tmx, conflicts) = rebase_tmx("", &ours, &theirs, "en", "fr");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
        assert!(tmx
            .get("Hi")
            .unwrap()
            .note
            .as_ref()
            .unwrap()
            .contains("Bonjour"));
    }

    #[test]
    fn duplicated_source_conflict_resolves_only_the_complete_entry_key() {
        fn key(file: &str, id: &str, prev: &str, next: &str, path: &str) -> EntryKeyDto {
            EntryKeyDto {
                file: file.into(),
                source_text: "Repeated source".into(),
                id: Some(id.into()),
                prev: Some(prev.into()),
                next: Some(next.into()),
                path: Some(path.into()),
            }
        }
        fn alternative(key: &EntryKeyDto, translation: &str) -> TmxEntry {
            TmxEntry {
                source: key.source_text.clone(),
                translation: translation.into(),
                default_translation: false,
                file: Some(key.file.clone()),
                id: key.id.clone(),
                prev: key.prev.clone(),
                next: key.next.clone(),
                path: key.path.clone(),
                ..Default::default()
            }
        }
        fn xml(entries: impl IntoIterator<Item = TmxEntry>) -> String {
            let mut tmx = ProjectTmx::new();
            entries.into_iter().for_each(|entry| tmx.insert(entry));
            tmx.to_xml("en", "fr")
        }

        let wanted = key(
            "chapter/wanted.yaml",
            "wanted_0",
            "wanted before",
            "wanted after",
            "wanted",
        );
        let decoy = key(
            "chapter/decoy.yaml",
            "decoy_0",
            "decoy before",
            "decoy after",
            "decoy",
        );
        let base = xml([
            alternative(&wanted, "base wanted"),
            alternative(&decoy, "decoy stable"),
        ]);
        let ours = xml([
            alternative(&decoy, "decoy stable"),
            alternative(&wanted, "ours wanted"),
        ]);
        let theirs = xml([
            alternative(&wanted, "theirs wanted"),
            alternative(&decoy, "decoy stable"),
        ]);

        let (merged, conflicts) = crate::tmx_rebase::rebase_detailed(
            &base,
            &ours,
            &theirs,
            "en",
            "fr",
            &Default::default(),
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].source, "Repeated source");
        assert_eq!(conflicts[0].ours, "ours wanted");
        assert_eq!(conflicts[0].theirs, "theirs wanted");
        assert_eq!(conflicts[0].entry_key.as_ref(), Some(&wanted));
        assert_eq!(
            merged
                .get_multiple_translation_for_key(&wanted)
                .unwrap()
                .translation,
            "ours wanted"
        );
        assert_eq!(
            merged
                .get_multiple_translation_for_key(&decoy)
                .unwrap()
                .translation,
            "decoy stable"
        );

        let dir = tempfile::tempdir().unwrap();
        let props =
            ProjectProperties::create(dir.path().join("project"), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        merged.write(&props.save_tmx_path(), "en", "fr").unwrap();
        crate::team_settings::save_conflicts(&props, &conflicts).unwrap();

        let remaining =
            resolve_for_key(&props, "Repeated source", Some(&wanted), "theirs", None).unwrap();
        assert!(remaining.is_empty());
        let resolved = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(
            resolved
                .get_multiple_translation_for_key(&wanted)
                .unwrap()
                .translation,
            "theirs wanted"
        );
        assert_eq!(
            resolved
                .get_multiple_translation_for_key(&decoy)
                .unwrap()
                .translation,
            "decoy stable"
        );
    }

    #[test]
    fn cancellable_same_source_resolutions_rollback_and_advance_one_complete_key() {
        fn key(file: &str, id: &str, path: &str) -> EntryKeyDto {
            EntryKeyDto {
                file: file.into(),
                source_text: "Repeated source".into(),
                id: Some(id.into()),
                prev: Some(format!("{id} before")),
                next: Some(format!("{id} after")),
                path: Some(path.into()),
            }
        }
        fn alternative(key: &EntryKeyDto, translation: &str) -> TmxEntry {
            TmxEntry {
                source: key.source_text.clone(),
                translation: translation.into(),
                default_translation: false,
                file: Some(key.file.clone()),
                id: key.id.clone(),
                prev: key.prev.clone(),
                next: key.next.clone(),
                path: key.path.clone(),
                ..Default::default()
            }
        }
        fn conflict(key: &EntryKeyDto, ours: &str, theirs: &str) -> Conflict {
            Conflict {
                kind: "tmx".into(),
                source: key.source_text.clone(),
                ours: ours.into(),
                theirs: theirs.into(),
                message: format!("TMX conflict on {}", key.source_text),
                entry_key: Some(key.clone()),
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let props =
            ProjectProperties::create(dir.path().join("project"), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        let first = key("chapter/first.yaml", "first_0", "first");
        let second = key("chapter/second.yaml", "second_0", "second");
        let mut tmx = ProjectTmx::new();
        tmx.insert(alternative(&first, "ours first"));
        tmx.insert(alternative(&second, "ours second"));
        tmx.write(&props.save_tmx_path(), "en", "fr").unwrap();
        let conflicts = vec![
            conflict(&first, "ours first", "theirs first"),
            conflict(&second, "ours second", "theirs second"),
        ];
        crate::team_settings::save_conflicts(&props, &conflicts).unwrap();
        std::fs::write(props.source_dir.join("unrelated.txt"), "project tree stays").unwrap();

        let tmx_before = std::fs::read(props.save_tmx_path()).unwrap();
        let conflicts_before =
            std::fs::read(props.root.join(".repositories/prep/conflicts.json")).unwrap();
        let (stage_tx, stage_rx) = std::sync::mpsc::sync_channel(0);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(0);
        let resume_rx = std::sync::Mutex::new(resume_rx);
        let cancellation =
            omegat_core::cancellation::CancellationToken::with_checkpoint_observer(move |stage| {
                if stage == "team.resolve.writeback" {
                    stage_tx.send(()).unwrap();
                    resume_rx.lock().unwrap().recv().unwrap();
                }
            });
        let worker_cancellation = cancellation.clone();
        let worker_first = first.clone();
        let worker = std::thread::spawn(move || {
            resolve_for_key_cancellable(
                &props,
                "Repeated source",
                Some(&worker_first),
                "theirs",
                None,
                &worker_cancellation,
            )
        });
        stage_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("resolution did not reach TMX write-back");
        cancellation.cancel();
        resume_tx.send(()).unwrap();
        assert!(matches!(worker.join().unwrap(), Err(TeamError::Cancelled)));

        let root = dir.path().join("project");
        assert_eq!(
            std::fs::read(root.join("omegat/project_save.tmx")).unwrap(),
            tmx_before
        );
        assert_eq!(
            std::fs::read(root.join(".repositories/prep/conflicts.json")).unwrap(),
            conflicts_before
        );
        assert_eq!(
            std::fs::read_to_string(root.join("source/unrelated.txt")).unwrap(),
            "project tree stays"
        );
        assert!(!root.join(".repositories/prep/resolved.json").exists());
        assert!(!root.join(".repositories/transactions/active.json").exists());
        assert!(std::fs::read_dir(root.join(".repositories/transactions"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".snapshot")));

        let props = ProjectProperties::load(&root).unwrap();
        let remaining =
            resolve_for_key(&props, "Repeated source", Some(&first), "theirs", None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].entry_key.as_ref(), Some(&second));
        let after_first = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(
            after_first
                .get_multiple_translation_for_key(&first)
                .unwrap()
                .translation,
            "theirs first"
        );
        assert_eq!(
            after_first
                .get_multiple_translation_for_key(&second)
                .unwrap()
                .translation,
            "ours second"
        );

        let remaining =
            resolve_for_key(&props, "Repeated source", Some(&second), "ours", None).unwrap();
        assert!(remaining.is_empty());
        let after_second = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(
            after_second
                .get_multiple_translation_for_key(&first)
                .unwrap()
                .translation,
            "theirs first"
        );
        assert_eq!(
            after_second
                .get_multiple_translation_for_key(&second)
                .unwrap()
                .translation,
            "ours second"
        );
    }

    #[test]
    fn interrupted_resolution_restores_same_project_conflict_queue() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let props = ProjectProperties::create(root.clone(), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        let key = EntryKeyDto {
            file: "chapter/interrupted.yaml".into(),
            source_text: "Interrupted source".into(),
            id: Some("interrupted_0".into()),
            prev: Some("before".into()),
            next: Some("after".into()),
            path: Some("interrupted".into()),
        };
        let mut tmx = ProjectTmx::new();
        tmx.insert(TmxEntry {
            source: key.source_text.clone(),
            translation: "ours interrupted".into(),
            default_translation: false,
            file: Some(key.file.clone()),
            id: key.id.clone(),
            prev: key.prev.clone(),
            next: key.next.clone(),
            path: key.path.clone(),
            ..Default::default()
        });
        tmx.write(&props.save_tmx_path(), "en", "fr").unwrap();
        crate::team_settings::save_conflicts(
            &props,
            &[
                Conflict {
                    kind: "tmx".into(),
                    source: key.source_text.clone(),
                    ours: "ours interrupted".into(),
                    theirs: "theirs interrupted".into(),
                    message: "TMX conflict on Interrupted source".into(),
                    entry_key: Some(key.clone()),
                },
                Conflict {
                    kind: "glossary".into(),
                    source: "pending glossary".into(),
                    ours: "ours pending".into(),
                    theirs: "theirs pending".into(),
                    message: "glossary conflict on pending glossary".into(),
                    entry_key: None,
                },
            ],
        )
        .unwrap();
        let tmx_before = std::fs::read(props.save_tmx_path()).unwrap();
        let conflicts_before =
            std::fs::read(root.join(".repositories/prep/conflicts.json")).unwrap();

        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::team_resolution_crash_worker",
                "--nocapture",
            ])
            .env("OMEGAT_TEAM_RESOLUTION_CRASH_PROJECT", &root)
            .env(
                "OMEGAT_TEAM_RESOLUTION_CRASH_KEY",
                serde_json::to_string(&key).unwrap(),
            )
            .status()
            .unwrap();
        assert!(!status.success());
        let active = root.join(".repositories/transactions/active.json");
        assert!(active.is_file());
        assert_ne!(std::fs::read(props.save_tmx_path()).unwrap(), tmx_before);

        assert!(recover_interrupted_sync(&props).unwrap());
        assert!(!active.exists());
        assert_eq!(std::fs::read(props.save_tmx_path()).unwrap(), tmx_before);
        assert_eq!(
            std::fs::read(root.join(".repositories/prep/conflicts.json")).unwrap(),
            conflicts_before
        );
        assert_eq!(list_conflicts(&props).len(), 2);
        assert!(!root.join(".repositories/prep/resolved.json").exists());
    }

    #[test]
    fn team_resolution_crash_worker() {
        let (Ok(root), Ok(raw_key)) = (
            std::env::var("OMEGAT_TEAM_RESOLUTION_CRASH_PROJECT"),
            std::env::var("OMEGAT_TEAM_RESOLUTION_CRASH_KEY"),
        ) else {
            return;
        };
        let props = ProjectProperties::load(Path::new(&root)).unwrap();
        let key: EntryKeyDto = serde_json::from_str(&raw_key).unwrap();
        crate::rebase_and_commit::crash_after_resolution_writeback();
        resolve_for_key(&props, &key.source_text, Some(&key), "theirs", None).unwrap();
        panic!("resolution crash injection did not terminate the worker");
    }

    #[test]
    fn committed_resolution_receipt_survives_crash_without_rollback_or_replay() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let props = ProjectProperties::create(root.clone(), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        let key = EntryKeyDto {
            file: "atomic.txt".into(),
            source_text: "Atomic conflict".into(),
            id: Some("atomic_0".into()),
            prev: None,
            next: None,
            path: Some("atomic".into()),
        };
        let mut tmx = ProjectTmx::new();
        tmx.insert(TmxEntry {
            source: key.source_text.clone(),
            translation: "ours before commit".into(),
            default_translation: false,
            file: Some(key.file.clone()),
            id: key.id.clone(),
            prev: key.prev.clone(),
            next: key.next.clone(),
            path: key.path.clone(),
            ..Default::default()
        });
        tmx.write(&props.save_tmx_path(), "en", "fr").unwrap();
        crate::team_settings::save_conflicts(
            &props,
            &[Conflict {
                kind: "tmx".into(),
                source: key.source_text.clone(),
                ours: "ours before commit".into(),
                theirs: "theirs committed once".into(),
                message: "atomic conflict".into(),
                entry_key: Some(key.clone()),
            }],
        )
        .unwrap();

        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::team_resolution_committed_crash_worker",
                "--nocapture",
            ])
            .env("OMEGAT_TEAM_COMMITTED_CRASH_PROJECT", &root)
            .env(
                "OMEGAT_TEAM_COMMITTED_CRASH_KEY",
                serde_json::to_string(&key).unwrap(),
            )
            .status()
            .unwrap();
        assert!(!status.success());

        let active = root.join(".repositories/transactions/active.json");
        let committed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&active).unwrap()).unwrap();
        assert_eq!(committed["status"], "completed");
        assert_eq!(committed["payload"]["phase"], "committed");
        assert!(committed["payload"]["product_manifest"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|receipt| receipt["path"] == "project/omegat/project_save.tmx"));
        assert_eq!(
            committed["commit"]["manifest_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert!(committed["commit"]["manifest_items"].as_u64().unwrap() > 0);

        let saved = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(
            saved
                .get_multiple_translation_for_key(&key)
                .unwrap()
                .translation,
            "theirs committed once"
        );
        assert!(list_conflicts(&props).is_empty());

        assert!(!recover_interrupted_sync(&props).unwrap());
        assert!(!active.exists());
        let reopened = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(
            reopened
                .get_multiple_translation_for_key(&key)
                .unwrap()
                .translation,
            "theirs committed once"
        );
        assert!(list_conflicts(&props).is_empty());
    }

    #[test]
    fn team_resolution_committed_crash_worker() {
        let (Ok(root), Ok(raw_key)) = (
            std::env::var("OMEGAT_TEAM_COMMITTED_CRASH_PROJECT"),
            std::env::var("OMEGAT_TEAM_COMMITTED_CRASH_KEY"),
        ) else {
            return;
        };
        let props = ProjectProperties::load(Path::new(&root)).unwrap();
        let key: EntryKeyDto = serde_json::from_str(&raw_key).unwrap();
        crate::remote_repository_provider::crash_after_product_commit();
        resolve_for_key(&props, &key.source_text, Some(&key), "theirs", None).unwrap();
        panic!("committed resolution crash injection did not terminate the worker");
    }

    #[test]
    fn file_sync_copies_and_rebases() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("omegat")).unwrap();
        write_tmx(
            &remote.join("omegat").join("project_save.tmx"),
            &[("Hi", "Bonjour")],
        );
        let props = team_props(
            local.clone(),
            "file",
            &remote.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].ours, "Salut");
        assert_eq!(c[0].theirs, "Bonjour");
        assert_eq!(c[0].kind, "tmx");
    }

    #[test]
    fn file_sync_merges_different_segments_and_glossary() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("omegat")).unwrap();
        std::fs::create_dir_all(remote.join("glossary")).unwrap();
        write_tmx(
            &remote.join("omegat").join("project_save.tmx"),
            &[("Hi", "Bonjour")],
        );
        std::fs::write(remote.join("glossary").join("glossary.txt"), "cat\tchat\n").unwrap();
        let props = team_props(
            local.clone(),
            "file",
            &remote.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Bye", "Au revoir")]);
        std::fs::write(&props.glossary_file, "dog\tchien\n").unwrap();
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "sync");
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Bonjour");
        assert_eq!(tmx.get("Bye").unwrap().translation, "Au revoir");
        let gloss = std::fs::read_to_string(&props.glossary_file).unwrap();
        assert!(gloss.contains("cat\tchat"));
        assert!(gloss.contains("dog\tchien"));
    }

    #[test]
    fn glossary_conflict_is_structured_and_resolvable() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("glossary")).unwrap();
        std::fs::write(remote.join("glossary").join("glossary.txt"), "cat\tchat\n").unwrap();
        write_tmx(&remote.join("omegat").join("project_save.tmx"), &[]);
        let props = team_props(
            local,
            "file",
            &remote.to_string_lossy(),
            vec![default_mapping()],
        );
        std::fs::write(&props.glossary_file, "cat\tfelin\n").unwrap();
        write_tmx(&props.save_tmx_path(), &[]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let left = resolve(&props, "cat", "theirs", None).unwrap();
        assert!(left.is_empty());
        let gloss = std::fs::read_to_string(&props.glossary_file).unwrap();
        assert!(gloss.contains("cat\tchat"));
    }

    #[test]
    fn http_downloads_remote_tmx_into_rebase() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("mem.tmx");
        write_tmx(&remote, &[("Hi", "Bonjour")]);
        let local = dir.path().join("local");
        let url = format!("file://{}", remote.display());
        let props = team_props(
            local,
            "http",
            &url,
            vec![RepositoryMapping {
                local: "omegat/project_save.tmx".into(),
                repository: "project_save.tmx".into(),
                includes: vec![],
                excludes: vec![],
            }],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props);
        assert_eq!(c[0].theirs, "Bonjour");
        let left = resolve(&props, "Hi", "ours", None).unwrap();
        assert!(left.is_empty());
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
    }

    #[test]
    fn mapping_excludes_are_applied() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("source")).unwrap();
        std::fs::write(remote.join("source").join("keep.txt"), "keep").unwrap();
        std::fs::write(remote.join("source").join("skip.bak"), "skip").unwrap();
        let props = team_props(
            local,
            "file",
            &remote.to_string_lossy(),
            vec![RepositoryMapping {
                local: "/".into(),
                repository: "/".into(),
                includes: vec![],
                excludes: vec!["**/*.bak".into()],
            }],
        );
        write_tmx(&props.save_tmx_path(), &[]);
        sync(&props).unwrap();
        assert!(props.source_dir.join("keep.txt").exists());
        assert!(!props.source_dir.join("skip.bak").exists());
    }

    #[test]
    fn empty_repository_list_is_local() {
        let dir = tempfile::tempdir().unwrap();
        let props =
            ProjectProperties::create(dir.path().to_path_buf(), "en".into(), "fr".into(), false);
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "local");
    }

    #[test]
    fn credentials_controller_persists_user_pass() {
        let dir = tempfile::tempdir().unwrap();
        let props =
            ProjectProperties::create(dir.path().to_path_buf(), "en".into(), "fr".into(), false);
        crate::repositories_credentials_controller::upsert(
            &props,
            crate::repositories_credentials_panel::RepositoryCredentials {
                url: "https://example.com/repo.git".into(),
                user_pass: crate::user_pass_dialog::UserPass::new("u", "p"),
                passphrase: crate::passphrase_dialog::Passphrase::new("phrase"),
            },
        )
        .unwrap();
        let loaded = crate::repositories_credentials_controller::load(&props);
        let row = loaded.for_url("https://example.com/repo.git").unwrap();
        assert_eq!(row.user_pass.username, "u");
        assert_eq!(row.passphrase.value, "phrase");
    }

    fn seed_bare(bare: &Path, seed: &Path) {
        assert!(Command::new("git")
            .args(["init", "--bare", &bare.to_string_lossy()])
            .status()
            .unwrap()
            .success());
        std::fs::create_dir_all(seed.join("omegat")).unwrap();
        write_tmx(&seed.join("omegat").join("project_save.tmx"), &[]);
        std::fs::create_dir_all(seed.join("glossary")).unwrap();
        std::fs::write(seed.join("glossary").join("glossary.txt"), "").unwrap();
        std::fs::create_dir_all(seed.join("source")).unwrap();
        std::fs::write(seed.join("source").join("remote.txt"), "remote").unwrap();
        assert!(Command::new("git")
            .args(["init"])
            .current_dir(seed)
            .status()
            .unwrap()
            .success());
        let _ = Command::new("git")
            .args(["checkout", "-B", "main"])
            .current_dir(seed)
            .status();
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(seed)
            .status();
        crate::git_remote_repository2::commit(seed, "seed").unwrap();
        assert!(Command::new("git")
            .args(["remote", "add", "origin", &bare.to_string_lossy()])
            .current_dir(seed)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["push", "-u", "origin", "HEAD:refs/heads/main"])
            .current_dir(seed)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn git_two_clients_merge_different_segments() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for R6 two-client test");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let props_a = team_props(a, "git", &bare.to_string_lossy(), vec![default_mapping()]);
        write_tmx(&props_a.save_tmx_path(), &[("Hi", "Salut")]);
        let r = sync(&props_a).unwrap();
        assert_eq!(r.action, "sync");

        let props_b = team_props(b, "git", &bare.to_string_lossy(), vec![default_mapping()]);
        write_tmx(&props_b.save_tmx_path(), &[("Bye", "Au revoir")]);
        sync(&props_b).unwrap();
        let tmx_b = parse_tmx(
            &std::fs::read_to_string(props_b.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx_b.get("Hi").unwrap().translation, "Salut");
        assert_eq!(tmx_b.get("Bye").unwrap().translation, "Au revoir");

        sync(&props_a).unwrap();
        let tmx_a = parse_tmx(
            &std::fs::read_to_string(props_a.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx_a.get("Hi").unwrap().translation, "Salut");
        assert_eq!(tmx_a.get("Bye").unwrap().translation, "Au revoir");
    }

    #[test]
    fn git_two_clients_same_segment_conflicts_then_resolve() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for R6 conflict test");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let props_a = team_props(
            dir.path().join("a"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_a.save_tmx_path(), &[("Hi", "Salut")]);
        sync(&props_a).unwrap();

        let props_b = team_props(
            dir.path().join("b"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_b.save_tmx_path(), &[("Hi", "Bonjour")]);
        let err = sync(&props_b).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props_b);
        assert_eq!(c[0].ours, "Bonjour");
        assert_eq!(c[0].theirs, "Salut");
        resolve(&props_b, "Hi", "theirs", None).unwrap();
        let tmx = parse_tmx(
            &std::fs::read_to_string(props_b.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
        sync(&props_b).unwrap();
    }

    #[test]
    fn multi_repository_prepare_failure_restores_project_before_any_commit() {
        let dir = tempfile::tempdir().unwrap();
        let remote_a = dir.path().join("remote-a");
        let remote_b = dir.path().join("remote-b");
        std::fs::create_dir_all(&remote_a).unwrap();
        std::fs::create_dir_all(&remote_b).unwrap();
        std::fs::write(remote_a.join("first.txt"), "remote first").unwrap();
        std::fs::write(remote_b.join("child.txt"), "remote child").unwrap();

        let mut props = team_props(
            dir.path().join("project"),
            "file",
            &remote_a.to_string_lossy(),
            vec![RepositoryMapping {
                local: "source/first.txt".into(),
                repository: "first.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        );
        props.repositories.push(RepositoryDef {
            repo_type: "file".into(),
            url: remote_b.to_string_lossy().into_owned(),
            branch: None,
            mappings: vec![RepositoryMapping {
                local: "source/blocker/child.txt".into(),
                repository: "child.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        });
        props.write().unwrap();
        std::fs::write(props.source_dir.join("first.txt"), "local first").unwrap();
        std::fs::write(props.source_dir.join("blocker"), "local blocker").unwrap();

        let error = sync(&props).unwrap_err();
        assert!(matches!(error, TeamError::Io(_)), "{error:?}");
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("first.txt")).unwrap(),
            "local first"
        );
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("blocker")).unwrap(),
            "local blocker"
        );
        assert_eq!(
            std::fs::read_to_string(remote_a.join("first.txt")).unwrap(),
            "remote first"
        );
        assert_eq!(
            std::fs::read_to_string(remote_b.join("child.txt")).unwrap(),
            "remote child"
        );
    }

    #[test]
    fn multi_git_commit_failure_compensates_already_published_repository() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for multi-repository transaction test");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare_a = dir.path().join("remote-a.git");
        let bare_b = dir.path().join("remote-b.git");
        seed_bare(&bare_a, &dir.path().join("seed-a"));
        seed_bare(&bare_b, &dir.path().join("seed-b"));

        let mut props = team_props(
            dir.path().join("project"),
            "git",
            &bare_a.to_string_lossy(),
            vec![RepositoryMapping {
                local: "source/first.txt".into(),
                repository: "first.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        );
        props.repositories.push(RepositoryDef {
            repo_type: "git".into(),
            url: bare_b.to_string_lossy().into_owned(),
            branch: Some("main".into()),
            mappings: vec![RepositoryMapping {
                local: "source/second.txt".into(),
                repository: "second.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        });
        props.write().unwrap();
        std::fs::write(props.source_dir.join("first.txt"), "local first").unwrap();
        std::fs::write(props.source_dir.join("second.txt"), "local second").unwrap();

        let _fault_lock =
            crate::remote_repository_provider::lock_commit_fault_injection();
        crate::remote_repository_provider::fail_next_commit_for(1);
        let error = sync(&props).unwrap_err();
        assert!(matches!(error, TeamError::Command(_)), "{error:?}");
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("first.txt")).unwrap(),
            "local first"
        );
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("second.txt")).unwrap(),
            "local second"
        );

        let first = git2::Repository::open_bare(&bare_a).unwrap();
        let first_tree = first
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        assert!(first_tree.get_path(Path::new("first.txt")).is_err());
        let second = git2::Repository::open_bare(&bare_b).unwrap();
        let second_tree = second
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        assert!(second_tree.get_path(Path::new("second.txt")).is_err());
    }

    #[test]
    fn concurrent_git_writers_get_a_real_non_fast_forward_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let mapping = vec![RepositoryMapping {
            local: "source/race.txt".into(),
            repository: "race.txt".into(),
            includes: vec![],
            excludes: vec![],
        }];
        let props_a = team_props(
            dir.path().join("writer-a"),
            "git",
            &bare.to_string_lossy(),
            mapping.clone(),
        );
        let props_b = team_props(
            dir.path().join("writer-b"),
            "git",
            &bare.to_string_lossy(),
            mapping,
        );
        crate::remote_repository_factory::prepare(&props_a, &props_a.repositories[0]).unwrap();
        crate::remote_repository_factory::prepare(&props_b, &props_b.repositories[0]).unwrap();
        let version_a = get_version(&props_a, 0, "").unwrap().unwrap();
        let version_b = get_version(&props_b, 0, "").unwrap().unwrap();
        assert_eq!(version_a, version_b);
        std::fs::write(props_a.source_dir.join("race.txt"), "writer-a").unwrap();
        std::fs::write(props_b.source_dir.join("race.txt"), "writer-b").unwrap();
        copy_mapped(&props_a, &props_a.repositories[0], CopyDir::ProjectToRepo).unwrap();
        copy_mapped(&props_b, &props_b.repositories[0], CopyDir::ProjectToRepo).unwrap();

        let work_a =
            crate::project_team_settings::repo_work_dir(&props_a, &props_a.repositories[0]);
        let work_b =
            crate::project_team_settings::repo_work_dir(&props_b, &props_b.repositories[0]);
        let commit_a = crate::git2_ops::commit_if_changed(
            &work_a,
            Some(std::slice::from_ref(&version_a)),
            "concurrent writer a",
        )
        .unwrap()
        .unwrap();
        let commit_b = crate::git2_ops::commit_if_changed(
            &work_b,
            Some(std::slice::from_ref(&version_b)),
            "concurrent writer b",
        )
        .unwrap()
        .unwrap();
        assert_ne!(commit_a, commit_b);
        let anonymous = crate::user_pass_dialog::UserPass::new("", "");
        crate::git2_ops::push(&work_a, "origin", "HEAD:refs/heads/main", &anonymous).unwrap();
        let rejection =
            crate::git2_ops::push(&work_b, "origin", "HEAD:refs/heads/main", &anonymous)
                .unwrap_err();
        assert!(matches!(rejection, TeamError::Command(_)));

        let remote = git2::Repository::open_bare(&bare).unwrap();
        let tree = remote
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        let blob = remote
            .find_blob(tree.get_path(Path::new("race.txt")).unwrap().id())
            .unwrap();
        assert_eq!(std::str::from_utf8(blob.content()).unwrap(), "writer-a");
    }

    #[test]
    fn interrupted_sync_is_recovered_from_persistent_journal() {
        let dir = tempfile::tempdir().unwrap();
        let bare_a = dir.path().join("remote-a.git");
        let bare_b = dir.path().join("remote-b.git");
        seed_bare(&bare_a, &dir.path().join("seed-a"));
        seed_bare(&bare_b, &dir.path().join("seed-b"));
        let mut props = team_props(
            dir.path().join("project"),
            "git",
            &bare_a.to_string_lossy(),
            vec![RepositoryMapping {
                local: "source/first.txt".into(),
                repository: "first.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        );
        props.repositories.push(RepositoryDef {
            repo_type: "git".into(),
            url: bare_b.to_string_lossy().into_owned(),
            branch: Some("main".into()),
            mappings: vec![RepositoryMapping {
                local: "source/second.txt".into(),
                repository: "second.txt".into(),
                includes: vec![],
                excludes: vec![],
            }],
        });
        props.write().unwrap();
        std::fs::write(props.source_dir.join("first.txt"), "local first").unwrap();
        std::fs::write(props.source_dir.join("second.txt"), "local second").unwrap();

        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::team_sync_crash_worker", "--nocapture"])
            .env("OMEGAT_TEAM_CRASH_PROJECT", &props.root)
            .status()
            .unwrap();
        assert_eq!(status.success(), false);
        let active = props.root.join(".repositories/transactions/active.json");
        assert_eq!(active.is_file(), true);

        assert_eq!(recover_interrupted_sync(&props).unwrap(), true);
        assert_eq!(active.exists(), false);
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("first.txt")).unwrap(),
            "local first"
        );
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("second.txt")).unwrap(),
            "local second"
        );
        for (bare, path) in [(&bare_a, "first.txt"), (&bare_b, "second.txt")] {
            let repo = git2::Repository::open_bare(bare).unwrap();
            let tree = repo
                .find_reference("refs/heads/main")
                .unwrap()
                .peel_to_tree()
                .unwrap();
            assert_eq!(tree.get_path(Path::new(path)).is_err(), true);
        }
        let history =
            std::fs::read_to_string(props.root.join(".repositories/transactions/history.ndjson"))
                .unwrap();
        let rows: Vec<serde_json::Value> = history
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows.last().unwrap()["payload"]["phase"], "recovered");
        assert_eq!(
            rows.iter().any(|row| {
                row["payload"]["phase"] == "publishing"
                    && row["payload"]["commit_started"] == serde_json::json!([0])
                    && row["payload"]["published"] == serde_json::json!([])
            }),
            true
        );
        assert_eq!(
            rows.iter().any(|row| {
                row["payload"]["phase"] == "recovering"
                    && row["payload"]["published"] == serde_json::json!([0])
            }),
            true
        );
    }

    #[test]
    fn mixed_git_and_file_compensation_and_renderer_ack_are_atomic() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for mixed repository transaction coverage");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let file_remote = dir.path().join("file-remote");
        let failure_remote = dir.path().join("failure-remote");
        std::fs::create_dir_all(file_remote.join("source")).unwrap();
        std::fs::create_dir_all(failure_remote.join("source")).unwrap();
        std::fs::write(file_remote.join("source/file.txt"), "file-before").unwrap();
        std::fs::write(failure_remote.join("source/block.txt"), "block-before").unwrap();

        let mapping = |local: &str| {
            vec![RepositoryMapping {
                local: format!("source/{local}"),
                repository: format!("source/{local}"),
                includes: vec![],
                excludes: vec![],
            }]
        };
        let mut props = team_props(
            dir.path().join("project"),
            "git",
            &bare.to_string_lossy(),
            mapping("remote.txt"),
        );
        props.repositories.push(RepositoryDef {
            repo_type: "file".into(),
            url: file_remote.to_string_lossy().into_owned(),
            branch: None,
            mappings: mapping("file.txt"),
        });
        props.repositories.push(RepositoryDef {
            repo_type: "file".into(),
            url: failure_remote.to_string_lossy().into_owned(),
            branch: None,
            mappings: mapping("block.txt"),
        });
        props.write().unwrap();
        for repo in &props.repositories {
            crate::remote_repository_factory::prepare(&props, repo).unwrap();
        }
        std::fs::write(props.source_dir.join("remote.txt"), "git-candidate").unwrap();
        std::fs::write(props.source_dir.join("file.txt"), "file-candidate").unwrap();
        std::fs::write(props.source_dir.join("block.txt"), "block-candidate").unwrap();

        let _fault_lock =
            crate::remote_repository_provider::lock_commit_fault_injection();
        crate::remote_repository_provider::fail_next_commit_for(2);
        let failed = commit_project_files_cancellable_scoped(
            &props,
            "source",
            &omegat_core::cancellation::CancellationToken::default(),
            15,
            Some("mixed-compensation"),
        )
        .unwrap_err();
        assert!(failed
            .to_string()
            .contains("injected repository 2 commit failure"));
        assert_eq!(
            std::fs::read_to_string(file_remote.join("source/file.txt")).unwrap(),
            "file-before"
        );
        assert_eq!(
            std::fs::read_to_string(failure_remote.join("source/block.txt")).unwrap(),
            "block-before"
        );

        let remote = git2::Repository::open_bare(&bare).unwrap();
        let remote_commit = remote
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let remote_tree = remote_commit.tree().unwrap();
        let remote_blob = remote
            .find_blob(
                remote_tree
                    .get_path(Path::new("source/remote.txt"))
                    .unwrap()
                    .id(),
            )
            .unwrap();
        assert_eq!(
            std::str::from_utf8(remote_blob.content()).unwrap(),
            "remote"
        );

        let work = crate::project_team_settings::repo_work_dir(&props, &props.repositories[0]);
        let work_repo = git2::Repository::open(&work).unwrap();
        assert_eq!(
            work_repo.head().unwrap().target(),
            Some(remote_commit.id()),
            "compensating Git HEAD was not published"
        );
        assert_eq!(
            std::fs::read_to_string(work.join("source/remote.txt")).unwrap(),
            "remote"
        );
        assert!(work_repo.statuses(None).unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("remote.txt")).unwrap(),
            "git-candidate",
            "rollback must preserve the user's pre-transaction project snapshot"
        );
        let active = props.root.join(".repositories/transactions/active.json");
        assert!(!active.exists());
        let failed_history =
            std::fs::read_to_string(props.root.join(".repositories/transactions/history.ndjson"))
                .unwrap();
        let failed_terminal: serde_json::Value =
            serde_json::from_str(failed_history.lines().last().unwrap()).unwrap();
        assert_eq!(failed_terminal["batch_id"], "mixed-compensation");
        assert_eq!(failed_terminal["status"], "cancelled");

        commit_project_files_cancellable_scoped(
            &props,
            "source",
            &omegat_core::cancellation::CancellationToken::default(),
            16,
            Some("mixed-receipt"),
        )
        .unwrap();
        let unacknowledged: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&active).unwrap()).unwrap();
        assert_eq!(unacknowledged["status"], "sidecar_committed");
        assert_eq!(unacknowledged["generation"], 16);
        assert_eq!(unacknowledged["batch_id"], "mixed-receipt");

        let adopted = pending_renderer_receipt(&props, 17).unwrap().unwrap();
        assert_eq!(adopted.generation, 17);
        assert_eq!(adopted.batch_id, "mixed-receipt");
        assert_eq!(adopted.status, TransactionStatus::SidecarCommitted);
        assert!(active.exists(), "unacknowledged receipt was compacted");

        let first_ack = acknowledge_renderer_receipt(&props, 17, "mixed-receipt").unwrap();
        assert!(first_ack.acknowledged);
        assert!(!first_ack.already_acknowledged);
        assert!(!active.exists());
        let history_after_first =
            std::fs::read(props.root.join(".repositories/transactions/history.ndjson")).unwrap();
        let duplicate = acknowledge_renderer_receipt(&props, 17, "mixed-receipt").unwrap();
        assert!(duplicate.acknowledged);
        assert!(duplicate.already_acknowledged);
        assert_eq!(
            std::fs::read(props.root.join(".repositories/transactions/history.ndjson")).unwrap(),
            history_after_first,
            "duplicate renderer ack appended or replayed product work"
        );
        assert_eq!(
            std::fs::read_to_string(file_remote.join("source/file.txt")).unwrap(),
            "file-candidate"
        );
        let committed_remote = git2::Repository::open_bare(&bare).unwrap();
        let committed_tree = committed_remote
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        let committed_blob = committed_remote
            .find_blob(
                committed_tree
                    .get_path(Path::new("source/remote.txt"))
                    .unwrap()
                    .id(),
            )
            .unwrap();
        assert_eq!(
            std::str::from_utf8(committed_blob.content()).unwrap(),
            "git-candidate"
        );
    }

    #[test]
    fn same_project_processes_are_serialized_by_transaction_lock() {
        let dir = tempfile::tempdir().unwrap();
        let props =
            ProjectProperties::create(dir.path().join("project"), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        let ready = dir.path().join("lock-ready");
        let release = dir.path().join("lock-release");
        let mut holder = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::team_project_lock_worker", "--nocapture"])
            .env("OMEGAT_TEAM_LOCK_PROJECT", &props.root)
            .env("OMEGAT_TEAM_LOCK_READY", &ready)
            .env("OMEGAT_TEAM_LOCK_RELEASE", &release)
            .spawn()
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.is_file() {
            assert!(
                Instant::now() < deadline,
                "child process did not acquire the team transaction lock"
            );
            assert_eq!(holder.try_wait().unwrap(), None);
            std::thread::sleep(Duration::from_millis(10));
        }

        let locked = sync(&props).unwrap_err();
        assert_eq!(
            locked.to_string(),
            format!(
                "conflict: team project is locked by another process: {}",
                props.root.display()
            )
        );
        assert_eq!(
            props
                .root
                .join(".repositories/transactions/active.json")
                .exists(),
            false
        );

        std::fs::write(&release, "").unwrap();
        assert_eq!(holder.wait().unwrap().success(), true);
        let report = sync(&props).unwrap();
        assert_eq!(report.action, "local");
        assert_eq!(report.message, "no repositories");
    }

    #[test]
    fn team_project_lock_worker() {
        let (Ok(root), Ok(ready), Ok(release)) = (
            std::env::var("OMEGAT_TEAM_LOCK_PROJECT"),
            std::env::var("OMEGAT_TEAM_LOCK_READY"),
            std::env::var("OMEGAT_TEAM_LOCK_RELEASE"),
        ) else {
            return;
        };
        let props = ProjectProperties::load(Path::new(&root)).unwrap();
        let _lock =
            crate::remote_repository_provider::acquire_project_transaction_lock(&props).unwrap();
        std::fs::write(ready, "").unwrap();
        while !Path::new(&release).is_file() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn team_sync_crash_worker() {
        let Ok(root) = std::env::var("OMEGAT_TEAM_CRASH_PROJECT") else {
            return;
        };
        let props = ProjectProperties::load(Path::new(&root)).unwrap();
        crate::remote_repository_provider::crash_after_publish_for(0);
        sync(&props).unwrap();
        panic!("crash injection did not terminate the worker");
    }

    #[test]
    fn git_provider_versions_guard_commit_and_switch() {
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let props = team_props(
            dir.path().join("client"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        crate::remote_repository_factory::prepare(&props, &props.repositories[0]).unwrap();
        let observed = get_version(&props, 0, "source/remote.txt")
            .unwrap()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(
                crate::project_team_settings::repo_work_dir(&props, &props.repositories[0])
                    .join("source/remote.txt")
            )
            .unwrap(),
            "remote"
        );

        std::fs::write(props.root.join("source/remote.txt"), "local").unwrap();
        copy_mapped(&props, &props.repositories[0], CopyDir::ProjectToRepo).unwrap();
        let committed =
            commit_after_version(&props, 0, &[Some(observed.clone())], "guarded update")
                .unwrap()
                .unwrap();
        assert_ne!(committed, observed);

        std::fs::write(props.root.join("source/remote.txt"), "stale").unwrap();
        copy_mapped(&props, &props.repositories[0], CopyDir::ProjectToRepo).unwrap();
        let error =
            commit_after_version(&props, 0, &[Some(observed.clone())], "stale update").unwrap_err();
        assert!(matches!(error, TeamError::Conflict(_)));

        switch_to_version(&props, 0, Some(&observed)).unwrap();
        assert_eq!(
            std::fs::read_to_string(
                crate::project_team_settings::repo_work_dir(&props, &props.repositories[0])
                    .join("source/remote.txt")
            )
            .unwrap(),
            "remote"
        );
    }

    #[test]
    fn git_sync_propagates_only_new_remote_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        let seed = dir.path().join("seed");
        seed_bare(&bare, &seed);
        let props = team_props(
            dir.path().join("client"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[]);
        sync(&props).unwrap();
        assert_eq!(
            std::fs::read_to_string(props.source_dir.join("remote.txt")).unwrap(),
            "remote"
        );

        crate::git2_ops::pull_ff(&seed, &crate::user_pass_dialog::UserPass::new("", "")).unwrap();
        std::fs::remove_file(seed.join("source/remote.txt")).unwrap();
        crate::git_remote_repository2::commit(&seed, "delete remote").unwrap();
        crate::git2_ops::push(
            &seed,
            "origin",
            "refs/heads/main:refs/heads/main",
            &crate::user_pass_dialog::UserPass::new("", ""),
        )
        .unwrap();

        sync(&props).unwrap();
        assert!(!props.source_dir.join("remote.txt").exists());
        assert_eq!(
            crate::git_remote_repository2::recently_deleted_files(&props, &props.repositories[0])
                .unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn http_two_clients_rebase_remote_tmx() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("mem.tmx");
        write_tmx(&remote, &[("Hi", "Bonjour"), ("Bye", "Au revoir")]);
        let url = format!("file://{}", remote.display());
        let mapping = vec![RepositoryMapping {
            local: "omegat/project_save.tmx".into(),
            repository: "project_save.tmx".into(),
            includes: vec![],
            excludes: vec![],
        }];
        let a = team_props(dir.path().join("a"), "http", &url, mapping.clone());
        let b = team_props(dir.path().join("b"), "http", &url, mapping);
        write_tmx(&a.save_tmx_path(), &[("Hi", "Salut")]);
        write_tmx(&b.save_tmx_path(), &[("Bye", "Ciao")]);
        let err_a = sync(&a).unwrap_err();
        assert!(matches!(err_a, TeamError::Conflict(_)), "{err_a:?}");
        let ca = list_conflicts(&a);
        assert_eq!(ca.len(), 1);
        assert_eq!(ca[0].source, "Hi");
        assert_eq!(ca[0].ours, "Salut");
        assert_eq!(ca[0].theirs, "Bonjour");
        let err_b = sync(&b).unwrap_err();
        assert!(matches!(err_b, TeamError::Conflict(_)), "{err_b:?}");
        let cb = list_conflicts(&b);
        assert_eq!(cb.len(), 1);
        assert_eq!(cb[0].source, "Bye");
        assert_eq!(cb[0].ours, "Ciao");
        assert_eq!(cb[0].theirs, "Au revoir");
    }

    #[test]
    #[ignore = "requires svn + svnadmin (STATUS: SVN product path is the svn binary)"]
    fn svn_checkout_update_commit() {
        if !which("svn") || !which("svnadmin") {
            panic!("svn/svnadmin not installed");
        }
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("svnrepo");
        run_cmd("svnadmin", None, &["create", &repo.to_string_lossy()]).unwrap();
        let url = format!("file://{}", repo.display());
        let seed = dir.path().join("seed");
        run_cmd("svn", None, &["checkout", &url, &seed.to_string_lossy()]).unwrap();
        std::fs::create_dir_all(seed.join("omegat")).unwrap();
        write_tmx(&seed.join("omegat").join("project_save.tmx"), &[]);
        let _ = run_cmd("svn", Some(&seed), &["add", "omegat"]);
        run_cmd("svn", Some(&seed), &["commit", "-m", "seed"]).unwrap();

        let props = team_props(
            dir.path().join("client"),
            "svn",
            &url,
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "sync");
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
    }

    fn remaining_golden(name: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/goldens/remaining")
            .join(name);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            v["exported_by"].as_str(),
            Some("org.omegat.tools.ExportGoldens")
        );
        v
    }

    #[test]
    fn team_slash_helpers_match_java() {
        let g = remaining_golden("RemoteRepositoryProvider2Test-testWithoutSlashes.json");
        for pair in g["cases"].as_array().unwrap() {
            assert_eq!(
                without_slashes(pair[0].as_str().unwrap()),
                pair[1].as_str().unwrap()
            );
        }
        let sl = remaining_golden("RemoteRepositoryProvider2Test-testWithSlashes.json");
        for pair in sl["cases"].as_array().unwrap() {
            assert_eq!(
                with_slashes(pair[0].as_str().unwrap()),
                pair[1].as_str().unwrap()
            );
        }
        let lead = remaining_golden("RemoteRepositoryProvider2Test-testWithLeadingSlash.json");
        for pair in lead["cases"].as_array().unwrap() {
            assert_eq!(
                with_leading_slash(pair[0].as_str().unwrap()),
                pair[1].as_str().unwrap()
            );
        }
        let rel = remaining_golden(
            "RemoteRepositoryProvider2Test-testRelativeRemoteToAbsoluteLocal.json",
        );
        let base = std::env::temp_dir();
        let got = relative_remote_to_absolute_local("file.txt", &base, "/", "/");
        assert_eq!(
            got.strip_prefix(&base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            rel["file"].as_str().unwrap()
        );
        let mapped =
            relative_remote_to_absolute_local("somedir/file.txt", &base, "somedir", "source");
        assert_eq!(
            mapped
                .strip_prefix(&base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            rel["mapped"].as_str().unwrap()
        );
    }

    #[test]
    fn http_retrieve_file_url_matches_java() {
        let g =
            remaining_golden("HTTPRemoteRepositoryTest-testRetrieveRetrievesFileSuccessfully.json");
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("remote.txt");
        std::fs::write(&src, g["body"].as_str().unwrap()).unwrap();
        let dest = dir.path().join("out.txt");
        crate::http_remote_repository::download(&format!("file://{}", src.display()), &dest)
            .unwrap();
        assert_eq!(dest.exists(), g["exists"].as_bool().unwrap());
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            g["body"].as_str().unwrap()
        );
    }

    #[test]
    fn http_switch_and_304_match_java() {
        let throws = remaining_golden(
            "HTTPRemoteRepositoryTest-testSwitchToVersionThrowsExceptionWhenVersionIsNotNull.json",
        );
        let err = crate::http_remote_repository::switch_to_version(throws["version"].as_str())
            .unwrap_err();
        assert_eq!(throws["throws"].as_bool().unwrap(), true);
        let TeamError::Command(message) = err else {
            panic!("expected command error");
        };
        assert_eq!(message, throws["message"].as_str().unwrap());
        let ok =
            remaining_golden("HTTPRemoteRepositoryTest-testSwitchToVersionUpdatesToLatest.json");
        assert_eq!(
            crate::http_remote_repository::switch_to_version(ok["version"].as_str()).is_ok(),
            ok["ok"].as_bool().unwrap()
        );
        let nm = remaining_golden(
            "HTTPRemoteRepositoryTest-testRetrieveHandlesNotModifiedResponse.json",
        );
        assert_eq!(
            crate::http_remote_repository::retrieve_skips_write(
                nm["status"].as_u64().unwrap() as u16
            ),
            nm["skip_write"].as_bool().unwrap()
        );
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.txt");
        std::fs::write(&dest, "Existing content").unwrap();
        crate::http_remote_repository::retrieve_with_status(
            304,
            &dest,
            "Existing content",
            "Test file contents",
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "Existing content");
    }

    #[test]
    fn remaining_copy_and_rename_match_java() {
        for name in [
            "RemoteRepositoryProviderTest-testCopyFileFromReposToProject.json",
            "RemoteRepositoryProviderTest-testCopyFileFromProjectToRepos.json",
            "RemoteRepositoryProviderTest-testCopyRenamedFileFromRepoToProject.json",
            "RemoteRepositoryProviderTest-testCopyRenamedFileFromProjectToRepos.json",
            "RemoteRepositoryProviderTest-testCopySubFileFromProjectToRepos.json",
            "RemoteRepositoryProviderTest-testCopyDirFromProjectToReposWithExcludes.json",
            "RemoteRepositoryProviderTest-testCopyDirFromProjectToReposWithExcludesWithDirectorySeparatorPrefix.json",
            "RemoteRepositoryProviderTest-testCopyAndDeletePropagateReposToProject.json",
        ] {
            let g = remaining_golden(name);
            let dir = tempfile::tempdir().unwrap();
            let remote = dir.path().join("remote");
            let local = dir.path().join("local");
            std::fs::create_dir_all(remote.join("source")).unwrap();
            std::fs::write(remote.join("source").join("file1.txt"), "one").unwrap();
            std::fs::write(remote.join("renamed.txt"), "renamed").unwrap();
            std::fs::write(remote.join("skip.bak"), "bak").unwrap();
            let mut mapping = default_mapping();
            if name.contains("Renamed") {
                mapping.local = "source/otherproject/file.txt".into();
                mapping.repository = "renamed.txt".into();
            }
            mapping.excludes = vec!["**/*.bak".into()];
            let props = team_props(local, "file", &remote.to_string_lossy(), vec![mapping]);
            let wc = crate::project_team_settings::repo_work_dir(&props, &props.repositories[0]);
            crate::team_utils::copy_tree(&remote, &wc, false).unwrap();
            let dir = if name.contains("FromProjectToRepos") {
                crate::mapping::CopyDir::ProjectToRepo
            } else {
                crate::mapping::CopyDir::RepoToProject
            };
            if matches!(dir, crate::mapping::CopyDir::ProjectToRepo) {
                std::fs::create_dir_all(props.root.join("source")).unwrap();
                std::fs::write(props.root.join("source").join("file1.txt"), "one").unwrap();
            }
            crate::mapping::copy_mapped(&props, &props.repositories[0], dir).unwrap();
            assert_eq!(g["copied"].as_bool().unwrap(), true);
            assert_eq!(g["excludes_honored"].as_bool().unwrap(), true);
        }
    }
}
