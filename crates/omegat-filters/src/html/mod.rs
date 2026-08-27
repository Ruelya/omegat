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
