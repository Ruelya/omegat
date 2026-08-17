use crate::error::Result;
use std::path::Path;

/// Import a MediaWiki dump or a folder of `.wiki` / `.txt` pages into `source/`.
pub fn import_wiki(src: &Path, project_source: &Path) -> Result<usize> {
    std::fs::create_dir_all(project_source)?;
    let mut n = 0;
    if src.is_file() {
        n += import_one(src, project_source)?;
    } else if src.is_dir() {
        for ent in walkdir::WalkDir::new(src).into_iter().flatten() {
            if ent.file_type().is_file() {
                n += import_one(ent.path(), project_source)?;
            }
        }
    }
    Ok(n)
}

fn import_one(path: &Path, dest_dir: &Path) -> Result<usize> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("page.txt");
    let dest = dest_dir.join(name);
    if dest.extension().and_then(|e| e.to_str()) == Some("xml") {
        let raw = std::fs::read_to_string(path)?;
        let text = extract_mediawiki_text(&raw);
        std::fs::write(dest.with_extension("txt"), text)?;
    } else {
        std::fs::copy(path, dest)?;
    }
    Ok(1)
}

pub fn extract_mediawiki_text(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(s) = rest.find("<text") {
        let after = &rest[s..];
        if let Some(gt) = after.find('>') {
            if let Some(end) = after[gt + 1..].find("</text>") {
                out.push_str(&after[gt + 1..gt + 1 + end]);
                out.push_str("\n\n");
                rest = &after[gt + 1 + end + 7..];
                continue;
            }
        }
        break;
    }
    if out.is_empty() {
        raw.to_string()
    } else {
        html_escape::decode_html_entities(&out).into_owned()
    }
}

/// MED project: a zip or folder containing `omegat.project` plus packages.
pub fn open_med(path: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    if path.is_dir() {
        for ent in walkdir::WalkDir::new(path).into_iter().flatten() {
            if ent.file_type().is_file() {
                let rel = ent.path().strip_prefix(path).unwrap_or(ent.path());
                let to = dest.join(rel);
                if let Some(p) = to.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(ent.path(), to)?;
            }
        }
        return Ok(());
    }
    std::fs::copy(path, dest.join(path.file_name().unwrap_or_default()))?;
    Ok(())
}

/// Convert a project directory layout (copy + rewrite `omegat.project` langs).
pub fn convert_project(src: &Path, dest: &Path, sl: &str, tl: &str) -> Result<()> {
    open_med(src, dest)?;
    let pf = dest.join("omegat.project");
    if pf.exists() {
        let mut raw = std::fs::read_to_string(&pf)?;
        raw = replace_tag(&raw, "source_lang", sl);
        raw = replace_tag(&raw, "target_lang", tl);
        std::fs::write(pf, raw)?;
    }
    Ok(())
}

fn replace_tag(raw: &str, tag: &str, value: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(s) = raw.find(&open) {
        if let Some(e) = raw[s..].find(&close) {
            let mut out = String::new();
            out.push_str(&raw[..s]);
            out.push_str(&open);
            out.push_str(value);
            out.push_str(&raw[s + e..]);
            return out;
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_mediawiki() {
        let xml = r#"<page><text>Hello &amp; world</text></page>"#;
        assert!(extract_mediawiki_text(xml).contains("Hello & world"));
    }
}
