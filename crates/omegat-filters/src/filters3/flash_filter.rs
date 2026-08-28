//! Java `FlashFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::flash_dialect::FlashDialect;

pub struct FlashFilter;

impl Filter for FlashFilter {
    fn id(&self) -> &'static str {
        "flash"
    }
    fn name(&self) -> &'static str {
        "Flash XML Export"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn file_supported(&self, path: &Path, _ctx: &FilterContext) -> bool {
        crate::read_to_string(path)
            .map(|raw| super::flash_dialect::file_looks_like(&raw))
            .unwrap_or(false)
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = FlashDialect::new();
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
        let dialect = FlashDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
