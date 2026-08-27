//! Java `IRemoteRepository2`.

use crate::error::Result;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

pub trait IRemoteRepository2 {
    fn repo_type(&self) -> &'static str;
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()>;
    fn commit(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()>;

    fn file_version(
        &self,
        _props: &ProjectProperties,
        _repo: &RepositoryDef,
        _file: &str,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    fn switch_to_version(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        version: Option<&str>,
    ) -> Result<()> {
        if version.is_some() {
            return Err(crate::error::TeamError::Command("Not supported".into()));
        }
        self.prepare(props, repo)
    }

    fn recently_deleted_files(
        &self,
        _props: &ProjectProperties,
        _repo: &RepositoryDef,
    ) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn commit_after_versions(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        _on_versions: &[Option<String>],
        _comment: &str,
    ) -> Result<Option<String>> {
        self.commit(props, repo)?;
        Ok(None)
    }
}
