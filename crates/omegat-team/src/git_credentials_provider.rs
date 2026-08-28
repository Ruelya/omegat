//! Java `GITCredentialsProvider`.

use crate::repositories_credentials_controller;
use crate::user_pass_dialog::UserPass;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

pub fn for_repo(props: &ProjectProperties, repo: &RepositoryDef) -> UserPass {
    repositories_credentials_controller::load(props)
        .for_url(&repo.url)
        .map(|r| r.user_pass.clone())
        .unwrap_or_default()
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
