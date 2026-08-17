//! Java `DocBookFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::docbook_dialect::DocBookDialect;

pub struct DocBookFilter;

impl Filter for DocBookFilter {
    fn id(&self) -> &'static str {
        "docbook"
    }
    fn name(&self) -> &'static str {
        "DocBook"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml", "*.dbk"]
    }
    fn phase(&self) -> u8 {
        4
    }

    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = DocBookDialect::new();
        let mut hooks = DefaultHooks::parse();
        parse_to_file(path, &dialect, &mut hooks)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = DocBookDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
