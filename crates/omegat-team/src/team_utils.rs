//! Java `TeamUtils`.

use crate::error::{Result, TeamError};
use omegat_core::cancellation::CancellationToken;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn strip_slash(s: &str) -> &str {
    s.trim_matches('/')
}

/// Java `RemoteRepositoryProvider.withoutSlashes`.
pub fn without_slashes(s: &str) -> String {
    strip_slash(s).to_string()
}

/// Java `RemoteRepositoryProvider.withSlashes`.
pub fn with_slashes(s: &str) -> String {
    format!("/{}/", strip_slash(s))
}

/// Java `RemoteRepositoryProvider.withLeadingSlash`.
pub fn with_leading_slash(s: &str) -> String {
    if s.starts_with('/') {
        s.to_string()
    } else {
        format!("/{s}")
    }
}

/// Java `RemoteRepositoryProvider.relativeRemoteToAbsoluteLocal`.
pub fn relative_remote_to_absolute_local(
    remote_file: &str,
    local_base: &Path,
    remote_prefix: &str,
    local_prefix: &str,
) -> PathBuf {
    let remote = without_slashes(remote_file);
    let rem_pref = without_slashes(remote_prefix);
    let loc_pref = without_slashes(local_prefix);
    let rel = if rem_pref.is_empty() {
        remote
    } else if let Some(rest) = remote.strip_prefix(&format!("{rem_pref}/")) {
        rest.to_string()
    } else if remote == rem_pref {
        String::new()
    } else {
        remote
    };
    let mut dest = if loc_pref.is_empty() {
        local_base.to_path_buf()
    } else {
        local_base.join(loc_pref)
    };
    if !rel.is_empty() {
        dest = dest.join(rel);
    }
    dest
}

pub fn join_mapped(base: &Path, mapped: &str) -> PathBuf {
    let rel = strip_slash(mapped);
    if rel.is_empty() {
        base.to_path_buf()
    } else {
        base.join(rel)
    }
}

pub fn join_rel(prefix: &str, rel: &str) -> String {
    let p = strip_slash(prefix);
    if p.is_empty() {
        rel.to_string()
    } else {
        format!("{p}/{rel}")
    }
}

pub fn rel_unix(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn sanitize_url(url: &str) -> String {
    let mut s = String::new();
    let mut prev_us = false;
    for c in url.chars() {
        if c.is_ascii_alphanumeric() || c == '.' {
            s.push(c);
            prev_us = false;
        } else if !prev_us {
            s.push('_');
            prev_us = true;
        }
    }
    if s.is_empty() {
        "repo".into()
    } else {
        s
    }
}

pub fn copy_tree(from: &Path, to: &Path, skip_vcs: bool) -> Result<()> {
    copy_tree_cancellable(from, to, skip_vcs, &CancellationToken::default(), None)
}

pub fn copy_tree_cancellable(
    from: &Path,
    to: &Path,
    skip_vcs: bool,
    cancellation: &CancellationToken,
    checkpoint: Option<&'static str>,
) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for ent in walkdir::WalkDir::new(from).into_iter().flatten() {
        let rel = ent.path().strip_prefix(from).unwrap_or(ent.path());
        let unix = rel.to_string_lossy().replace('\\', "/");
        if skip_vcs
            && (unix.starts_with(".git/")
                || unix == ".git"
                || unix.starts_with(".svn/")
                || unix.starts_with(".repositories/"))
        {
            continue;
        }
        if ent.file_type().is_dir() {
            std::fs::create_dir_all(to.join(rel))?;
            continue;
        }
        if !ent.file_type().is_file() {
            continue;
        }
        if checkpoint.map_or_else(
            || cancellation.is_cancelled(),
            |stage| cancellation.checkpoint(stage),
        ) {
            return Err(TeamError::Cancelled);
        }
        let dest = to.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(ent.path(), dest)?;
    }
    if cancellation.is_cancelled() {
        return Err(TeamError::Cancelled);
    }
    Ok(())
}

pub fn run_cmd(program: &str, dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut c = Command::new(program);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    let out = c
        .args(args)
        .output()
        .map_err(|e| TeamError::Command(format!("{program}: {e}")))?;
    if !out.status.success() {
        return Err(TeamError::Command(format!(
            "{program} {} : {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn which(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}

pub fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}
