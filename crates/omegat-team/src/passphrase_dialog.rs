//! Java `PassphraseDialog` — SSH / team passphrase.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Passphrase {
    pub value: String,
}

impl Passphrase {
    pub fn new(value: impl Into<String>) -> Self {
        Self { value: value.into() }
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}
