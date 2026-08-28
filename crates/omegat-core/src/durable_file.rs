// SPDX-License-Identifier: GPL-3.0-or-later

//! Same-directory atomic file replacement for config and project word lists.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static REPLACEMENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Replace `path` only after the complete candidate has reached stable storage.
///
/// The temporary lives beside the destination so the rename cannot cross a
/// filesystem boundary. Any write, file-sync, or rename error leaves the old
/// destination untouched; syncing the parent closes the rename durability gap.
pub fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("durable file has no parent: {}", path.display()),
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let sequence = REPLACEMENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{filename}.{}.{sequence}.tmp",
        std::process::id()
    ));

    let candidate = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = candidate {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_is_complete_and_leaves_no_candidate() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        std::fs::write(&path, b"before").unwrap();
        replace(&path, b"after\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"after\n");
        assert_eq!(
            std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>(),
            vec![std::ffi::OsString::from("settings.json")]
        );
    }

    #[test]
    fn rename_failure_preserves_candidate_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("occupied");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("member"), b"before").unwrap();

        let error = replace(&destination, b"after").unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::IsADirectory | ErrorKind::PermissionDenied | ErrorKind::AlreadyExists
        ));
        assert_eq!(
            std::fs::read(destination.join("member")).unwrap(),
            b"before"
        );
        assert_eq!(
            std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>(),
            vec![std::ffi::OsString::from("occupied")]
        );
    }
}
