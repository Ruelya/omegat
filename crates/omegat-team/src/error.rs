use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TeamError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("command failed: {0}")]
    Command(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unsupported repository type: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, TeamError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub kind: String,
    pub source: String,
    pub ours: String,
    pub theirs: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub action: String,
    pub message: String,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
}
