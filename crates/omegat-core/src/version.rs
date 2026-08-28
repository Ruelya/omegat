//! Java `org.omegat.util.VersionChecker`.

/// Java `VersionChecker.compareVersions`.
pub fn compare_versions(v1: &str, u1: &str, v2: &str, u2: &str) -> Result<i32, VersionError> {
    let a = version_numbers(v1, u1)?;
    let b = version_numbers(v2, u2)?;
    compare_lists(&a, &b)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    Length,
    Parse,
}

fn version_numbers(version: &str, update: &str) -> Result<Vec<i32>, VersionError> {
    let mut out = Vec::new();
    for n in version.split('.') {
        out.push(n.parse().map_err(|_| VersionError::Parse)?);
    }
    out.push(update.parse().map_err(|_| VersionError::Parse)?);
    Ok(out)
}

fn compare_lists(a: &[i32], b: &[i32]) -> Result<i32, VersionError> {
    if a.len() != b.len() {
        return Err(VersionError::Length);
    }
    for (x, y) in a.iter().zip(b) {
        if x < y {
            return Ok(-1);
        }
        if x > y {
            return Ok(1);
        }
    }
    Ok(0)
}
