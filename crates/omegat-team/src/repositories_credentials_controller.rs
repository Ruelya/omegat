//! Java `RepositoriesCredentialsController`.

use crate::error::Result;
use crate::project_team_settings::{credentials_path, prep_dir};
use crate::repositories_credentials_panel::{CredentialsPanel, RepositoryCredentials};
use omegat_core::properties::ProjectProperties;

pub fn load(props: &ProjectProperties) -> CredentialsPanel {
    crate::team_utils::read_json(&credentials_path(props)).unwrap_or_default()
}

pub fn save(props: &ProjectProperties, panel: &CredentialsPanel) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    std::fs::write(
        credentials_path(props),
        serde_json::to_string_pretty(panel).unwrap(),
    )?;
    Ok(())
}

pub fn upsert(props: &ProjectProperties, row: RepositoryCredentials) -> Result<CredentialsPanel> {
    let mut panel = load(props);
    if let Some(existing) = panel.rows.iter_mut().find(|r| r.url == row.url) {
        *existing = row;
    } else {
        panel.rows.push(row);
    }
    save(props, &panel)?;
    Ok(panel)
}
