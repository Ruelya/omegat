//! Java `XHTMLFilter`.

use crate::xml_filter::{engine_config, parse_to_file_cfg, write_xml_cfg, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::xhtml_dialect::XhtmlDialect;

pub struct XhtmlFilter;

impl Filter for XhtmlFilter {
    fn id(&self) -> &'static str {
        "xhtml"
    }
    fn name(&self) -> &'static str {
        "XHTML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xhtml", "*.html"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = XhtmlDialect::new(&ctx.options);
        let mut hooks = DefaultHooks::parse();
        parse_to_file_cfg(path, &dialect, &mut hooks, engine_config(ctx))
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = XhtmlDialect::new(&ctx.options);
        let mut hooks = DefaultHooks::write(translations);
        write_xml_cfg(source_path, dest_path, &dialect, &mut hooks, engine_config(ctx))
    }
}
