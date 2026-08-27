//! Java `last_entry.properties` — restore the last edited segment index.

use crate::consts::{DEFAULT_INTERNAL, LAST_ENTRY};
use std::path::Path;

pub fn load_last_index(project_root: &Path) -> usize {
    let path = project_root.join(DEFAULT_INTERNAL).join(LAST_ENTRY);
    if let Ok(raw) = std::fs::read_to_string(path) {
        for line in raw.lines() {
            if let Some(v) = line.strip_prefix("last_entry=") {
                return v.parse().unwrap_or(0);
            }
        }
    }
    0
}

pub fn save_last_index(project_root: &Path, index: usize) -> std::io::Result<()> {
    let path = project_root.join(DEFAULT_INTERNAL).join(LAST_ENTRY);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("last_entry={index}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_last_entry_properties() {
        let dir = tempfile::tempdir().unwrap();
        save_last_index(dir.path(), 7).unwrap();
        assert_eq!(load_last_index(dir.path()), 7);
        let raw = std::fs::read_to_string(dir.path().join("omegat").join("last_entry.properties")).unwrap();
        assert_eq!(raw, "last_entry=7\n");
    }
}
