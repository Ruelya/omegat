//! Java `Typo3Filter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::typo3_dialect::Typo3Dialect;

pub struct Typo3Filter;

impl Filter for Typo3Filter {
    fn id(&self) -> &'static str {
        "typo3"
    }
    fn name(&self) -> &'static str {
        "Typo3 LocManager"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = Typo3Dialect::new();
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
        let dialect = Typo3Dialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
