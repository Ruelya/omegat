//! Java `TXMLFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::txml_dialect::TXMLDialect;

pub struct TXMLFilter;

impl Filter for TXMLFilter {
    fn id(&self) -> &'static str {
        "txml"
    }
    fn name(&self) -> &'static str {
        "Wordfast TXML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.txml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = TXMLDialect::new();
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
        let dialect = TXMLDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
