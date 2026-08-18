//! Java `org.omegat.util.FileUtil`.

use crate::string_util::get_tail_segments;
use regex::Regex;
use std::path::{Path, PathBuf};

/// Java `RE_ABSOLUTE_WINDOWS` / `RE_ABSOLUTE_LINUX`.
pub fn is_relative(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let linux = Regex::new(r"^/.*").unwrap();
    let windows = Regex::new(r"^[A-Za-z]:(/.*)").unwrap();
    !linux.is_match(&path) && !windows.is_match(&path)
}

/// Java `FileUtil.absoluteForSystem`.
pub fn absolute_for_system(path: &str) -> String {
    let path = path.replace('\\', "/");
    let windows = Regex::new(r"^[A-Za-z]:(/.*)").unwrap();
    if let Some(c) = windows.captures(&path) {
        if !cfg!(windows) {
            return c.get(1).map(|m| m.as_str().to_string()).unwrap_or(path);
        }
    }
    path
}

/// Java `FileUtil.compileFileMask` (package-private, exported for goldens).
pub fn compile_file_mask(mask: &str) -> String {
    let mut mask = mask.to_string();
    if !mask.starts_with('/') {
        mask = format!("**/{mask}");
    }
    if mask.ends_with('/') {
        mask.push_str("**");
    }
    let chars: Vec<char> = mask.chars().collect();
    let mut m = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let cp = chars[i];
        if cp.is_ascii_alphanumeric() {
            m.push(cp);
        } else if cp == '/' {
            let rest: String = chars[i..].iter().collect();
            if rest.starts_with("/**/") {
                m.push_str("(?:/|/.*/)");
                i += 3;
            } else if rest.starts_with("/**") {
                m.push_str("(?:|/.*)");
                i += 2;
            } else {
                m.push(cp);
            }
        } else if cp == '?' {
            m.push_str("[^/]");
        } else if cp == '*' {
            let rest: String = chars[i..].iter().collect();
            if rest.starts_with("**/") {
                m.push_str("(?:|.*/)");
                i += 2;
            } else if rest.starts_with("**") {
                m.push_str(".*");
                i += 1;
            } else {
                m.push_str("[^/]*");
            }
        } else {
            m.push('\\');
            m.push(cp);
        }
        i += 1;
    }
    m
}

pub fn file_mask_matches(pattern: &str, path: &str) -> bool {
    let re = Regex::new(&format!("^{}$", compile_file_mask(pattern))).unwrap_or_else(|_| Regex::new("$^").unwrap());
    re.is_match(path)
}

/// Apache-style normalize used by `getUniqueNames` (unix separators, no trailing slash).
pub fn normalize_no_end_separator(path: &str) -> Option<String> {
    let raw = path.replace('\\', "/");
    if raw.starts_with("//") || raw.starts_with("../") || raw == ".." {
        return None;
    }
    let absolute = raw.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in raw.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            if parts.pop().is_none() {
                return None;
            }
            continue;
        }
        parts.push(seg);
    }
    if parts.is_empty() {
        return if absolute { Some("/".into()) } else { Some(String::new()) };
    }
    let mut out = parts.join("/");
    if absolute {
        out = format!("/{out}");
    }
    Some(out)
}

/// Java `FileUtil.getUniqueNames`.
pub fn get_unique_names(paths: &[String]) -> Vec<String> {
    let full: Vec<String> = paths
        .iter()
        .map(|p| normalize_no_end_separator(p).unwrap_or_default())
        .collect();
    let mut working: Vec<String> = full.iter().map(|p| get_tail_segments(p, '/', 1)).collect();
    if working.len() == 1 && !working[0].is_empty() {
        return working;
    }
    let mut segments = vec![1usize; full.len()];
    loop {
        let mut did_trim = false;
        let counts: Vec<usize> = working
            .iter()
            .map(|w| working.iter().filter(|x| *x == w).count())
            .collect();
        for i in 0..counts.len() {
            if counts[i] > 1 {
                let curr = working[i].clone();
                segments[i] += 1;
                let trimmed = get_tail_segments(&full[i], '/', segments[i]);
                if curr != trimmed {
                    working[i] = trimmed;
                    did_trim = true;
                }
            }
        }
        if !did_trim {
            break;
        }
    }
    for i in 0..working.len() {
        if working[i].is_empty() {
            working[i] = paths[i].clone();
        }
    }
    working
}

pub fn get_eol(bytes: &[u8]) -> Option<&'static str> {
    if bytes.windows(2).any(|w| w == b"\r\n") {
        return Some("\r\n");
    }
    if bytes.contains(&b'\r') {
        return Some("\r");
    }
    if bytes.contains(&b'\n') {
        return Some("\n");
    }
    None
}

pub fn get_eol_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    get_eol(&bytes).map(|s| s.to_string())
}

pub fn get_backup_filename(original: &Path, last_modified_millis: i64) -> String {
    let name = original.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let secs = last_modified_millis / 1000;
    let dt = chrono_like(secs);
    format!("{name}.{dt}.bak")
}

fn chrono_like(unix_secs: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let Some(t) = UNIX_EPOCH.checked_add(Duration::from_secs(unix_secs.max(0) as u64)) else {
        return "197001010000".into();
    };
    let Ok(d) = t.duration_since(UNIX_EPOCH) else {
        return "197001010000".into();
    };
    // UTC YYYYMMDDHHmm — FileUtilTest uses timezone UTC.
    let secs = d.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let (y, m, day) = civil_from_days(days as i64);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    format!("{y:04}{m:02}{day:02}{hh:02}{mm:02}")
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 24);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

pub fn compute_relative_path(root: &Path, file: &Path) -> std::io::Result<String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let rel = file.strip_prefix(&root).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "not under root")
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

pub fn expand_tilde_home_dir(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest).to_string_lossy().into();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_matches_java() {
        assert!(!is_relative("C:\\zz"));
        assert!(!is_relative("z:/zz"));
        assert!(!is_relative("c:\\zz"));
        assert!(is_relative("1:/zz"));
        assert!(!is_relative("/zz"));
        assert!(!is_relative("\\zz"));
        assert!(is_relative("zz/"));
    }

    #[test]
    fn unique_names_match_java() {
        assert_eq!(
            get_unique_names(&["/foo/foo.txt".into(), "/foo/bar.txt".into(), "/bar/baz.txt".into()]),
            vec!["foo.txt", "bar.txt", "baz.txt"]
        );
        assert_eq!(
            get_unique_names(&["/foo/foo.txt".into(), "/foo/bar.txt".into(), "/bar/bar.txt".into()]),
            vec!["foo.txt", "foo/bar.txt", "bar/bar.txt"]
        );
        assert_eq!(
            get_unique_names(&["foo/".into(), "bar/boo/../baz".into(), "/buz/baz".into(), "/baz//baz".into()]),
            vec!["foo", "bar/baz", "buz/baz", "baz/baz"]
        );
        assert_eq!(
            get_unique_names(&["//foo".into(), "../foo".into()]),
            vec!["//foo", "../foo"]
        );
    }

    #[test]
    fn compile_mask_java_sample() {
        assert_eq!(compile_file_mask("Ab1-&*/**"), r"(?:|.*/)Ab1\-\&[^/]*(?:|/.*)");
        assert!(file_mask_matches("*.txt", "/foo.txt"));
        assert!(file_mask_matches("*.txt", "/bar/foo.txt"));
        assert!(!file_mask_matches("*.txt", "/foo.txty"));
        assert!(file_mask_matches("/*.txt", "/foo.txt"));
        assert!(!file_mask_matches("/*.txt", "/bar/foo.txt"));
        assert!(file_mask_matches("**/test/**", "test"));
        assert!(!file_mask_matches("**/test/**", "/foo/tests/bar"));
        assert!(!file_mask_matches("foo/**/bar", "foobar"));
    }
}
