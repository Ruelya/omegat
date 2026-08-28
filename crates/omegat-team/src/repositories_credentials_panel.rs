//! Java `RepositoriesCredentialsPanel` — stored credential rows.

use crate::passphrase_dialog::Passphrase;
use crate::user_pass_dialog::UserPass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryCredentials {
    pub url: String,
    pub user_pass: UserPass,
    pub passphrase: Passphrase,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialsPanel {
    pub rows: Vec<RepositoryCredentials>,
}

impl CredentialsPanel {
    pub fn for_url(&self, url: &str) -> Option<&RepositoryCredentials> {
        self.rows.iter().find(|r| r.url == url)
    }
}
