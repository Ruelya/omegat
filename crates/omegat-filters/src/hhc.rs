//! Java `org.omegat.filters2.hhc.HHCFilter2` — Name param values only.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct HhcFilter;

impl Filter for HhcFilter {
    fn id(&self) -> &'static str {
        "hhc"
    }
    fn name(&self) -> &'static str {
        "HTML Help Compiler"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.hhc", "*.hhk"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

/// Java `HTMLWriter.flush` charset meta injection (`HTML_HEAD` = `<head[^e]*?>`).
fn inject_html_charset_meta(contents: &str, encoding: &str) -> String {
    let eol = if contents.contains("\r\n") {
        "\r\n"
    } else if contents.contains('\r') {
        "\r"
    } else {
        "\n"
    };
    let html_meta = format!(
        "<meta http-equiv=\"content-type\" content=\"text/html; charset={encoding}\">"
    );
    let re_enc = regex::Regex::new(
        r#"(?i)<meta[^>]+http-equiv\s*=\s*["']?content-type["']?[^>]*>"#,
    )
    .unwrap();
    let re_enc5 = regex::Regex::new(r#"(?i)<meta\s+charset\s*=[^>]*>"#).unwrap();
    let re_head = regex::Regex::new(r"(?i)<head[^e]*?>").unwrap();
    let re_html = regex::Regex::new(r"(?i)<html.*?>").unwrap();
    if re_enc.is_match(contents) {
        re_enc.replace(contents, html_meta.as_str()).into_owned()
    } else if re_enc5.is_match(contents) {
        re_enc5
            .replace(contents, format!("<meta charset=\"{encoding}\">").as_str())
            .into_owned()
    } else if let Some(m) = re_head.find(contents) {
        let mut out = String::new();
        out.push_str(&contents[..m.end()]);
        out.push_str(eol);
        out.push_str("    ");
        out.push_str(&html_meta);
        out.push_str(&contents[m.end()..]);
        out
    } else if let Some(m) = re_html.find(contents) {
        let mut out = String::new();
        out.push_str(&contents[..m.end()]);
        out.push_str(eol);
        out.push_str("<head>");
        out.push_str(eol);
        out.push_str("    ");
        out.push_str(&html_meta);
        out.push_str(eol);
        out.push_str("</head>");
        out.push_str(&contents[m.end()..]);
        out
    } else {
        format!("<html>{eol}<head>{eol}    {html_meta}{eol}</head>{eol}{contents}")
    }
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let re = Regex::new(r#"(?i)(<param\s+name\s*=\s*"Name"\s+value\s*=\s*")([^"]*)(")"#).unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut last = 0usize;
    for cap in re.captures_iter(raw) {
        let full = cap.get(0).unwrap();
        let value = cap.get(2).unwrap().as_str();
        written.push_str(&raw[last..full.start()]);
        written.push_str(&cap[1]);
        let id = segments.len().to_string();
        segments.push(seg(&id, value));
        let trans = if let Some(map) = translations {
            map.get(&id)
                .cloned()
                .or_else(|| map.get(value).cloned())
                .unwrap_or_else(|| value.to_string())
        } else {
            value.to_string()
        };
        written.push_str(&trans);
        written.push_str(&cap[3]);
        last = full.end();
    }
    written.push_str(&raw[last..]);
    written = inject_html_charset_meta(&written, "UTF-8");
    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}
