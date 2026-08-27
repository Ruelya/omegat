//! Product-path Git via libgit2 (`git2`), not the `git` binary.
//!
//! Covers clone / fetch / reset / commit / push plus a credential callback,
//! matching Java `GITRemoteRepository2` + `GITCredentialsProvider`.

use crate::error::{Result, TeamError};
use crate::user_pass_dialog::UserPass;
use git2::{
    build::RepoBuilder, Cred, CredentialType, FetchOptions, IndexAddOption, PushOptions,
    RemoteCallbacks, Repository, ResetType, Signature,
};
use std::path::Path;

fn map_err(e: git2::Error) -> TeamError {
    TeamError::Command(format!("git2: {e}"))
}

fn callbacks(user: &UserPass) -> RemoteCallbacks<'static> {
    let username = user.username.clone();
    let password = user.password.clone();
    let mut cb = RemoteCallbacks::new();
    cb.credentials(move |url, username_from_url, allowed| {
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) && !username.is_empty() {
            return Cred::userpass_plaintext(&username, &password);
        }
        if allowed.contains(CredentialType::DEFAULT) {
            if let Ok(c) = Cred::default() {
                return Ok(c);
            }
        }
        if let Some(u) = username_from_url {
            if allowed.contains(CredentialType::SSH_KEY) {
                if let Ok(c) = Cred::ssh_key_from_agent(u) {
                    return Ok(c);
                }
            }
        }
        let _ = url;
        Cred::default()
    });
    cb
}

fn fetch_opts(user: &UserPass) -> FetchOptions<'static> {
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(user));
    fo.download_tags(git2::AutotagOption::Unspecified);
    fo
}

pub fn init(dir: &Path) -> Result<Repository> {
    Repository::init(dir).map_err(map_err)
}

pub fn open(dir: &Path) -> Result<Repository> {
    Repository::open(dir).map_err(map_err)
}

pub fn clone(url: &str, dest: &Path, branch: Option<&str>, user: &UserPass) -> Result<Repository> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        let _ = std::fs::remove_dir_all(dest);
    }
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts(user));
    if let Some(b) = branch {
        builder.branch(b);
    }
    builder.clone(url, dest).map_err(map_err)
}

pub fn fetch(dir: &Path, remote: &str, user: &UserPass) -> Result<()> {
    let repo = open(dir)?;
    let mut rem = repo.find_remote(remote).map_err(map_err)?;
    rem.fetch(&[] as &[&str], Some(&mut fetch_opts(user)), None)
        .map_err(map_err)
}

pub fn reset_hard(dir: &Path, spec: &str) -> Result<()> {
    let repo = open(dir)?;
    let obj = repo.revparse_single(spec).map_err(map_err)?;
    repo.reset(&obj, ResetType::Hard, None).map_err(map_err)
}

pub fn has_ref(dir: &Path, spec: &str) -> bool {
    let Ok(repo) = open(dir) else {
        return false;
    };
    let ok = repo.revparse_single(spec).is_ok();
    drop(repo);
    ok
}

pub fn current_branch(dir: &Path) -> Result<String> {
    let repo = open(dir)?;
    let head = repo.head().map_err(map_err)?;
    head.shorthand()
        .map(str::to_string)
        .ok_or_else(|| TeamError::Command("git2: HEAD is detached".into()))
}

pub fn current_version(dir: &Path) -> Result<String> {
    let repo = open(dir)?;
    let commit = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(map_err)?;
    Ok(commit.id().to_string())
}

pub fn add_all(dir: &Path) -> Result<()> {
    let repo = open(dir)?;
    let mut index = repo.index().map_err(map_err)?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(map_err)?;
    index.write().map_err(map_err)
}

/// Commit the staged worktree when it differs from HEAD.
///
/// Java `GITRemoteRepository2.commit` returns `null` when the index has no
/// changes. Keeping that distinction prevents an unnecessary push and lets the
/// caller enforce the version observed before rebase.
pub fn commit_if_changed(
    dir: &Path,
    on_versions: Option<&[String]>,
    message: &str,
) -> Result<Option<String>> {
    let repo = open(dir)?;
    let mut index = repo.index().map_err(map_err)?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(map_err)?;
    index.write().map_err(map_err)?;
    let oid = index.write_tree().map_err(map_err)?;
    let tree = repo.find_tree(oid).map_err(map_err)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    if let Some(p) = &parent {
        if let Some(versions) = on_versions {
            let expected = versions
                .iter()
                .filter(|version| !version.is_empty())
                .any(|version| version == &p.id().to_string());
            if versions.iter().any(|version| !version.is_empty()) && !expected {
                return Err(TeamError::Conflict(format!(
                    "git version changed from [{}] to {}",
                    versions.join(", "),
                    p.id()
                )));
            }
        }
        if p.tree_id() == tree.id() {
            return Ok(None);
        }
    }

    let sig = repo
        .signature()
        .or_else(|_| Signature::now("OmegaT", "omegat@example.com"))
        .map_err(map_err)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let id = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(map_err)?;
    Ok(Some(id.to_string()))
}

pub fn commit(dir: &Path, message: &str) -> Result<String> {
    match commit_if_changed(dir, None, message)? {
        Some(version) => Ok(version),
        None => current_version(dir),
    }
}

pub fn push(dir: &Path, remote: &str, refspec: &str, user: &UserPass) -> Result<()> {
    let repo = open(dir)?;
    let mut rem = repo.find_remote(remote).map_err(map_err)?;
    let mut po = PushOptions::new();
    po.remote_callbacks(callbacks(user));
    rem.push(&[refspec], Some(&mut po)).map_err(map_err)
}

pub fn pull_ff(dir: &Path, user: &UserPass) -> Result<()> {
    fetch(dir, "origin", user)?;
    let branch = current_branch(dir)?;
    let spec = format!("refs/remotes/origin/{branch}");
    if !has_ref(dir, &spec) {
        return Err(TeamError::Command(format!(
            "git2: remote branch origin/{branch} was not fetched"
        )));
    }
    reset_hard(dir, &spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anonymous() -> UserPass {
        UserPass::new("", "")
    }

    #[test]
    fn clone_update_delete_commit_and_version_guard_use_git2() {
        let temp = tempfile::tempdir().unwrap();
        let seed_dir = temp.path().join("seed");
        let remote_dir = temp.path().join("remote.git");
        let clone_dir = temp.path().join("clone");

        let seed = init(&seed_dir).unwrap();
        seed.set_head("refs/heads/main").unwrap();
        std::fs::write(seed_dir.join("tracked.txt"), "one").unwrap();
        let first = commit(&seed_dir, "seed").unwrap();
        let remote = Repository::init_bare(&remote_dir).unwrap();
        drop(remote);
        seed.remote("origin", remote_dir.to_str().unwrap()).unwrap();
        push(
            &seed_dir,
            "origin",
            "refs/heads/main:refs/heads/main",
            &anonymous(),
        )
        .unwrap();

        clone(
            remote_dir.to_str().unwrap(),
            &clone_dir,
            Some("main"),
            &anonymous(),
        )
        .unwrap();
        assert_eq!(current_branch(&clone_dir).unwrap(), "main");
        assert_eq!(current_version(&clone_dir).unwrap(), first);

        std::fs::remove_file(clone_dir.join("tracked.txt")).unwrap();
        let deleted = commit_if_changed(
            &clone_dir,
            Some(std::slice::from_ref(&first)),
            "delete tracked",
        )
        .unwrap()
        .unwrap();
        assert_ne!(deleted, first);
        let cloned = open(&clone_dir).unwrap();
        let tree = cloned.head().unwrap().peel_to_tree().unwrap();
        assert!(tree.get_name("tracked.txt").is_none());
        drop(tree);
        drop(cloned);

        std::fs::write(clone_dir.join("new.txt"), "two").unwrap();
        let err = commit_if_changed(
            &clone_dir,
            Some(&["0000000000000000000000000000000000000000".into()]),
            "must not commit",
        )
        .unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        assert_eq!(current_version(&clone_dir).unwrap(), deleted);
    }
}
