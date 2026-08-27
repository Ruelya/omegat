//! Java `PreparedFileInfo`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreparedFileInfo {
    pub path: PathBuf,
    pub revision: String,
}
