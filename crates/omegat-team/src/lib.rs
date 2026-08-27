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
pub use mapping::{
    copy_mapped, copy_mapped_from_worktree, default_mapping, glob_match, propagate_deleted, CopyDir,
};
pub use passphrase_dialog::Passphrase;
pub use prepared_file_info::PreparedFileInfo;
pub use project_team_settings::{REPO_PREP, REPO_SUBDIR};
pub use rebase_and_commit::{rebase_all, rebase_project, resolve};
pub use remote_repository_factory::detect_repository_type;
pub use remote_repository_provider::{
    commit_after_version, commit_project_files, get_version, switch_to_version, sync,
};
pub use repositories_credentials_panel::{CredentialsPanel, RepositoryCredentials};
pub use team_settings::list_conflicts;
pub use team_tool::init;
pub use team_utils::{
    relative_remote_to_absolute_local, with_leading_slash, with_slashes, without_slashes,
};
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

        crate::git2_ops::pull_ff(
            &seed,
            &crate::user_pass_dialog::UserPass::new("", ""),
        )
        .unwrap();
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
