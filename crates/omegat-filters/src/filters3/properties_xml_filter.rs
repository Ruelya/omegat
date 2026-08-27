//! Java `PropertiesXmlFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::properties_dialect::PropertiesDialect;

pub struct PropertiesXmlFilter;

impl Filter for PropertiesXmlFilter {
    fn id(&self) -> &'static str {
        "propxml"
    }
    fn name(&self) -> &'static str {
        "Java Properties XML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = PropertiesDialect::new();
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
        let dialect = PropertiesDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
