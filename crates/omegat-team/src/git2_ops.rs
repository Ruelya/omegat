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
    open(dir).ok().and_then(|r| r.revparse_single(spec).ok()).is_some()
}

pub fn current_branch(dir: &Path) -> Result<String> {
    let repo = open(dir)?;
    let head = repo.head().map_err(map_err)?;
    Ok(head.shorthand().unwrap_or("main").to_string())
}

pub fn add_all(dir: &Path) -> Result<()> {
    let repo = open(dir)?;
    let mut index = repo.index().map_err(map_err)?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(map_err)?;
    index.write().map_err(map_err)
}

pub fn commit(dir: &Path, message: &str) -> Result<String> {
    let repo = open(dir)?;
    let mut index = repo.index().map_err(map_err)?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(map_err)?;
    index.write().map_err(map_err)?;
    let oid = index.write_tree().map_err(map_err)?;
    let tree = repo.find_tree(oid).map_err(map_err)?;
    let sig = Signature::now("OmegaT", "omegat@example.com").map_err(map_err)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = match &parent {
        Some(c) => vec![c],
        None => vec![],
    };
    if let Some(p) = &parent {
        if p.tree_id() == tree.id() {
            return Ok(p.id().to_string());
        }
    }
    let id = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(map_err)?;
    Ok(id.to_string())
}

pub fn push(dir: &Path, remote: &str, refspec: &str, user: &UserPass) -> Result<()> {
    let repo = open(dir)?;
    let mut rem = repo.find_remote(remote).map_err(map_err)?;
    let mut po = PushOptions::new();
    po.remote_callbacks(callbacks(user));
    rem.push(&[refspec], Some(&mut po)).map_err(map_err)
}

pub fn pull_ff(dir: &Path, user: &UserPass) -> Result<()> {
    let _ = fetch(dir, "origin", user);
    let branch = current_branch(dir).unwrap_or_else(|_| "main".into());
    let spec = format!("refs/remotes/origin/{branch}");
    if has_ref(dir, &spec) {
        reset_hard(dir, &spec)?;
    }
    Ok(())
}
