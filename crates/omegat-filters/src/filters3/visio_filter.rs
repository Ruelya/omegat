//! Java `VisioFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::visio_dialect::VisioDialect;

pub struct VisioFilter;

impl Filter for VisioFilter {
    fn id(&self) -> &'static str {
        "visio"
    }
    fn name(&self) -> &'static str {
        "Visio"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.vdx", "*.vsdx"]
    }
    fn phase(&self) -> u8 {
        4
    }

    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = VisioDialect::new();
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
        let dialect = VisioDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
