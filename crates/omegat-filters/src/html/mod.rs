//! Java `org.omegat.filters2.html2` — FilterVisitor + HTMLOptions + HTMLWriter.

mod filter_visitor;
mod html_options;
mod html_writer;
mod tokenizer;

pub use filter_visitor::{process_html, VisitorKind};
pub use html_writer::entities_to_chars;

use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct HtmlFilter;

impl Filter for HtmlFilter {
    fn id(&self) -> &'static str {
        "html"
    }
    fn name(&self) -> &'static str {
        "HTML and XHTML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.html", "*.htm", "*.xhtml", "*.xht"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process_html(&read_to_string(path)?, ctx, VisitorKind::Html, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let out = process_html(
            &read_to_string(source_path)?,
            ctx,
            VisitorKind::Html,
            Some(translations),
        )
        .written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn utf16_bytes(text: &str, little_endian: bool) -> Vec<u8> {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(if little_endian {
                &unit.to_le_bytes()
            } else {
                &unit.to_be_bytes()
            });
        }
        bytes
    }

    #[test]
    fn html_filter_reads_utf16_bom_boundaries() {
        let dir = tempdir().unwrap();
        for (name, little_endian) in [("little.html", true), ("big.html", false)] {
            let path = dir.path().join(name);
            std::fs::write(
                &path,
                utf16_bytes("<html><body><p>Zażółć 😀</p></body></html>", little_endian),
            )
            .unwrap();
            let parsed = HtmlFilter.parse(&path, &FilterContext::default()).unwrap();
            let sources: Vec<_> = parsed
                .segments
                .iter()
                .map(|segment| segment.source.as_str())
                .collect();
            assert_eq!(sources, vec!["Zażółć 😀"], "{name}");
        }
    }
}
