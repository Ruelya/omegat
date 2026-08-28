//! Java `SVNAuthenticationManager`.

use crate::repositories_credentials_controller;
use crate::user_pass_dialog::UserPass;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

pub fn for_repo(props: &ProjectProperties, repo: &RepositoryDef) -> UserPass {
    repositories_credentials_controller::load(props)
        .for_url(&repo.url)
        .map(|r| r.user_pass.clone())
        .unwrap_or_default()
}

pub fn svn_auth_args(user: &UserPass) -> Vec<String> {
    if user.username.is_empty() {
        return vec![];
    }
    let mut args = vec![
        "--username".into(),
        user.username.clone(),
        "--non-interactive".into(),
    ];
    if !user.password.is_empty() {
        args.push("--password".into());
        args.push(user.password.clone());
    }
    args
}
