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
mod user_pass_dialog;

pub use error::{Conflict, SyncReport, TeamError};
pub use mapping::default_mapping;
pub use passphrase_dialog::Passphrase;
pub use prepared_file_info::PreparedFileInfo;
pub use project_team_settings::{REPO_PREP, REPO_SUBDIR};
pub use rebase_and_commit::{rebase_all, rebase_project, resolve};
pub use remote_repository_factory::detect_repository_type;
pub use remote_repository_provider::{commit_project_files, sync};
pub use repositories_credentials_panel::{CredentialsPanel, RepositoryCredentials};
pub use team_settings::list_conflicts;
pub use team_tool::init;
pub use tmx_rebase::rebase_tmx;
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
    use omegat_core::tmx::parse_tmx;
    use std::path::{Path, PathBuf};
    use std::process::Command;

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
        assert_eq!(detect_repository_type("svn://example.com/repo"), Some("svn"));
        assert_eq!(detect_repository_type("git://example.com/repo"), Some("git"));
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
        let _ = Command::new("git").args(["add", "-A"]).current_dir(seed).status();
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
        let props_a = team_props(
            a,
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_a.save_tmx_path(), &[("Hi", "Salut")]);
        let r = sync(&props_a).unwrap();
        assert_eq!(r.action, "sync");

        let props_b = team_props(
            b,
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
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
}
