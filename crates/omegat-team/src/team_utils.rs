//! Java `TeamUtils`.

use crate::error::{Result, TeamError};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn strip_slash(s: &str) -> &str {
    s.trim_matches('/')
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
    std::fs::create_dir_all(to)?;
    for ent in walkdir::WalkDir::new(from).into_iter().flatten() {
        if !ent.file_type().is_file() {
            continue;
        }
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
        let dest = to.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(ent.path(), dest)?;
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
