//! Java `XMLSpreadsheetFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::xmlspreadsheet_dialect::XMLSpreadsheetDialect;

pub struct XMLSpreadsheetFilter;

impl Filter for XMLSpreadsheetFilter {
    fn id(&self) -> &'static str {
        "xmlss"
    }
    fn name(&self) -> &'static str {
        "XML Spreadsheet 2003"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = XMLSpreadsheetDialect::new();
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
        let dialect = XMLSpreadsheetDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
