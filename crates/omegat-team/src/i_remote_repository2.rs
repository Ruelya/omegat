//! Java `IRemoteRepository2`.

use crate::error::Result;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

pub trait IRemoteRepository2 {
    fn repo_type(&self) -> &'static str;
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()>;
    fn commit(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()>;
}
