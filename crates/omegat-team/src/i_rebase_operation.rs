//! Java `IRebaseOperation`.

use crate::error::{Conflict, Result};
use omegat_core::properties::ProjectProperties;
use std::collections::HashSet;

pub trait IRebaseOperation {
    fn rebase(
        &self,
        props: &ProjectProperties,
        resolved: &HashSet<String>,
    ) -> Result<Vec<Conflict>>;
}
