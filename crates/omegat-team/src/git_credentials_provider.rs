//! Java `GITCredentialsProvider`.

use crate::repositories_credentials_controller;
use crate::user_pass_dialog::UserPass;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

const KEY_TYPES: [&str; 4] = ["RSA", "DSA", "ECDSA", "EDDSA"];

pub fn for_repo(props: &ProjectProperties, repo: &RepositoryDef) -> UserPass {
    repositories_credentials_controller::load(props)
        .for_url(&repo.url)
        .map(|r| r.user_pass.clone())
        .unwrap_or_default()
}

/// Extract the host-key fingerprint from JGit/libssh prompts.
///
/// This follows Java `GITCredentialsProvider.extractFingerprint`: legacy
/// 16-byte colon-delimited fingerprints, direct SHA-256 prompts, and the
/// two-line EC MD5/SHA-256 prompt are accepted. Unknown prompt shapes fail
/// closed so a credential callback cannot approve an unparsed host key.
pub fn extract_fingerprint(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if !lines.iter().any(|line| {
        line.starts_with("The authenticity of host '")
            && (line.ends_with("' can't be established.")
                || line.ends_with("' cannot be established."))
    }) {
        return None;
    }
    if !lines
        .iter()
        .any(|line| *line == "Are you sure you want to continue connecting?")
        && !lines
            .iter()
            .any(|line| *line == "Accept and store this key, and continue connecting?")
    {
        return None;
    }

    for key_type in KEY_TYPES {
        let prefix = format!("{key_type} key fingerprint is ");
        for line in &lines {
            let Some(value) = line
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix('.'))
            else {
                continue;
            };
            if valid_md5_fingerprint(value) {
                return Some(value.to_string());
            }
        }
    }
    for key_type in KEY_TYPES {
        let prefix = format!("{key_type} key fingerprint is SHA256:");
        for line in &lines {
            let Some(value) = line
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix('.'))
            else {
                continue;
            };
            if valid_sha256_fingerprint(value) {
                return Some(value.to_string());
            }
        }
    }
    for window in lines.windows(3) {
        if window[0] != "The EC key's fingerprints are:"
            || !window[1].starts_with("MD5:")
            || !valid_md5_fingerprint(&window[1]["MD5:".len()..])
        {
            continue;
        }
        let Some(value) = window[2].strip_prefix("SHA256:") else {
            continue;
        };
        if valid_sha256_fingerprint(value) {
            return Some(value.to_string());
        }
    }
    None
}

fn valid_md5_fingerprint(value: &str) -> bool {
    let pairs: Vec<&str> = value.split(':').collect();
    pairs.len() == 16
        && pairs.iter().all(|pair| {
            pair.len() == 2
                && pair
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn valid_sha256_fingerprint(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+'))
}

/// Extra `git -c` args when a username/password is stored.
pub fn git_config_args(user: &UserPass) -> Vec<String> {
    if user.is_empty() {
        return vec![];
    }
    vec![
        "-c".into(),
        format!("credential.username={}", user.username),
    ]
}
