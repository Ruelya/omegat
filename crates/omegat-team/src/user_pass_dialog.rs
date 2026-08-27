//! Java `UserPassDialog` — typed credentials (UI lives in the desktop prefs page).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserPass {
    pub username: String,
    pub password: String,
}

impl UserPass {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.username.is_empty() && self.password.is_empty()
    }
}
