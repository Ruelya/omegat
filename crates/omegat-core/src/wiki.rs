use crate::error::{CoreError, Result};
use std::fs::File;
use std::io::{self, Write};
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
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("page.txt");
    let dest = dest_dir.join(name);
    let ext = dest.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "xml" || name.ends_with(".xml") {
        let raw = std::fs::read_to_string(path)?;
        let pages = extract_mediawiki_pages(&raw);
        if pages.is_empty() {
            std::fs::write(dest.with_extension("txt"), extract_mediawiki_text(&raw))?;
            return Ok(1);
        }
        let mut n = 0;
        for (title, text) in pages {
            let file = sanitize_filename(&title);
            std::fs::write(dest_dir.join(format!("{file}.txt")), text)?;
            n += 1;
        }
        return Ok(n);
    }
    std::fs::copy(path, dest)?;
    Ok(1)
}

pub fn extract_mediawiki_text(raw: &str) -> String {
    extract_mediawiki_pages(raw)
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join("\n\n")
        .pipe_if_empty(raw)
}

fn extract_tag_inner<'a>(block: &'a str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let s = block.find(&open)?;
    let after = &block[s..];
    let gt = after.find('>')?;
    let end = after[gt + 1..].find(&close)?;
    Some(html_escape::decode_html_entities(&after[gt + 1..gt + 1 + end]).into_owned())
}

pub fn extract_mediawiki_pages(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(s) = rest.find("<page") {
        let after = &rest[s..];
        let end = after.find("</page>").unwrap_or(after.len());
        let page = &after[..end];
        let title =
            extract_tag_inner(page, "title").unwrap_or_else(|| format!("page{}", out.len() + 1));
        if let Some(text) = extract_tag_inner(page, "text") {
            if !text.trim().is_empty() {
                out.push((title, text));
            }
        }
        rest = if end < after.len() {
            &after[end + 7..]
        } else {
            ""
        };
    }
    out
}

fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().replace(' ', "_");
    if s.is_empty() {
        "page".into()
    } else {
        s
    }
}

trait PipeIfEmpty {
    fn pipe_if_empty(self, raw: &str) -> String;
}

impl PipeIfEmpty for String {
    fn pipe_if_empty(self, raw: &str) -> String {
        if self.is_empty() {
            raw.to_string()
        } else {
            self
        }
    }
}

/// MED project: unzip a package into a project tree, or copy a folder.
pub fn open_med(path: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    if path.is_dir() {
        copy_tree(path, dest)?;
        return Ok(());
    }
    let is_zip = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip") || e.eq_ignore_ascii_case("med"))
        .unwrap_or(false);
    if is_zip || looks_like_zip(path) {
        return unzip_to(path, dest);
    }
    // A loose omegat.project (or any file) is not a project tree.
    if path.file_name().and_then(|s| s.to_str()) == Some("omegat.project") {
        if let Some(parent) = path.parent() {
            return copy_tree(parent, dest);
        }
    }
    Err(CoreError::InvalidProject(format!(
        "MED/convert source must be a project folder or zip, not a single opaque file: {}",
        path.display()
    )))
}

fn looks_like_zip(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    bytes.starts_with(b"PK")
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    for ent in walkdir::WalkDir::new(from).into_iter().flatten() {
        if !ent.file_type().is_file() {
            continue;
        }
        let rel = ent.path().strip_prefix(from).unwrap_or(ent.path());
        let dest = to.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(ent.path(), dest)?;
    }
    Ok(())
}

fn unzip_to(src: &Path, dest: &Path) -> Result<()> {
    let file = File::open(src)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| CoreError::InvalidProject(e.to_string()))?;
    for i in 0..archive.len() {
        let mut zf = archive
            .by_index(i)
            .map_err(|e| CoreError::InvalidProject(e.to_string()))?;
        let name = zf.name().to_string();
        if name.contains("..") {
            continue;
        }
        let out = dest.join(&name);
        if zf.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut destf = File::create(&out)?;
        io::copy(&mut zf, &mut destf)?;
        destf.flush()?;
    }
    Ok(())
}

/// Convert a project directory/zip (copy + rewrite `omegat.project` langs and filter hints).
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
    fn extracts_mediawiki_pages() {
        let xml = r#"<mediawiki><page><title>Hello/Page</title><text>Hello &amp; world</text></page></mediawiki>"#;
        let pages = extract_mediawiki_pages(xml);
        assert_eq!(pages[0].0, "Hello/Page");
        assert!(pages[0].1.contains("Hello & world"));
    }

    #[test]
    fn med_unzip_lays_out_project() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("proj");
        std::fs::create_dir_all(src.join("source")).unwrap();
        std::fs::write(
            src.join("omegat.project"),
            "<source_lang>en</source_lang><target_lang>fr</target_lang>",
        )
        .unwrap();
        std::fs::write(src.join("source").join("a.txt"), "hi").unwrap();
        let zip_path = dir.path().join("pack.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::FileOptions::default();
            zw.start_file("omegat.project", opts).unwrap();
            zw.write_all(b"<source_lang>en</source_lang><target_lang>de</target_lang>")
                .unwrap();
            zw.start_file("source/a.txt", opts).unwrap();
            zw.write_all(b"hi").unwrap();
            zw.finish().unwrap();
        }
        let dest = dir.path().join("out");
        convert_project(&zip_path, &dest, "es", "it").unwrap();
        assert!(dest.join("source").join("a.txt").exists());
        let proj = std::fs::read_to_string(dest.join("omegat.project")).unwrap();
        assert!(proj.contains("<source_lang>es</source_lang>"));
        assert!(proj.contains("<target_lang>it</target_lang>"));
        assert!(!dest.join("pack.zip").exists());
    }
}
