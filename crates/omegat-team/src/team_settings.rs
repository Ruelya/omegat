//! Java `TeamSettings` — persisted conflict / resolved lists under `.repositories/prep/`.

use crate::error::{Conflict, Result};
use crate::project_team_settings::{conflicts_path, prep_dir, resolved_path};
use crate::team_utils::read_json;
use omegat_core::cancellation::CancellationToken;
use omegat_core::properties::ProjectProperties;
use std::collections::HashSet;
use std::io::Write;

pub fn list_conflicts(props: &ProjectProperties) -> Vec<Conflict> {
    read_json(&conflicts_path(props)).unwrap_or_default()
}

pub fn save_conflicts(props: &ProjectProperties, conflicts: &[Conflict]) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    std::fs::write(
        conflicts_path(props),
        serde_json::to_string_pretty(conflicts).unwrap(),
    )?;
    Ok(())
}

pub fn save_conflicts_cancellable(
    props: &ProjectProperties,
    conflicts: &[Conflict],
    cancellation: &CancellationToken,
) -> Result<()> {
    struct CancellableWriter<'a> {
        file: std::fs::File,
        cancellation: &'a CancellationToken,
    }

    impl Write for CancellableWriter<'_> {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            if self.cancellation.is_cancelled() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "request cancelled",
                ));
            }
            self.file.write(buffer)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.file.flush()
        }
    }

    std::fs::create_dir_all(prep_dir(props))?;
    let file = std::fs::File::create(conflicts_path(props))?;
    let mut writer = CancellableWriter { file, cancellation };
    if let Err(error) = serde_json::to_writer_pretty(&mut writer, conflicts) {
        return if cancellation.is_cancelled() {
            Err(crate::error::TeamError::Cancelled)
        } else {
            Err(crate::error::TeamError::Command(error.to_string()))
        };
    }
    writer.flush()?;
    if cancellation.is_cancelled() {
        return Err(crate::error::TeamError::Cancelled);
    }
    Ok(())
}

pub fn read_resolved(props: &ProjectProperties) -> HashSet<String> {
    read_json::<Vec<String>>(&resolved_path(props))
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub fn mark_resolved(props: &ProjectProperties, source: &str) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    let mut v: Vec<String> = read_json(&resolved_path(props)).unwrap_or_default();
    if !v.iter().any(|s| s == source) {
        v.push(source.into());
    }
    let json = serde_json::to_string(&v)
        .map_err(|error| crate::error::TeamError::Command(error.to_string()))?;
    std::fs::write(resolved_path(props), json)?;
    Ok(())
}

pub fn clear_resolved(props: &ProjectProperties) {
    let _ = std::fs::remove_file(resolved_path(props));
}
