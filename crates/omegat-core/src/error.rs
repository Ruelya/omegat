use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("xml: {0}")]
    Xml(String),
    #[error("project not open")]
    ProjectNotOpen,
    #[error("optimistic lock failed for entry {0}")]
    OptimisticLock(usize),
    #[error("filter: {0}")]
    Filter(String),
    #[error("invalid project: {0}")]
    InvalidProject(String),
    #[error("tag validation failed: {0}")]
    TagValidation(String),
    #[error("unimplemented: {0}")]
    Unimplemented(String),
}

impl From<omegat_filters::FilterError> for CoreError {
    fn from(e: omegat_filters::FilterError) -> Self {
        Self::Filter(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
