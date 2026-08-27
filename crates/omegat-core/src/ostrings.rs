//! Build/version strings from Java `org.omegat.util.OStrings`.

/// Return the development-build suffix shown for branch checkouts.
///
/// Detached, release, and unfiltered IDE builds deliberately have no suffix.
pub fn dev_build_marker(revision: &str, branch: &str) -> String {
    if branch.is_empty() || branch == "HEAD" || branch == "@gitbranch@" {
        String::new()
    } else {
        format!("[{revision} @ {branch}]")
    }
}

#[cfg(test)]
mod tests {
    use super::dev_build_marker;

    #[test]
    fn branch_and_detached_markers() {
        assert_eq!(
            dev_build_marker("6d79ee8db", "master"),
            "[6d79ee8db @ master]"
        );
        assert_eq!(dev_build_marker("6d79ee8db", "HEAD"), "");
    }
}
