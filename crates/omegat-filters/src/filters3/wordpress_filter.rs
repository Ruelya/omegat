//! Java `WordpressFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::wordpress_dialect::WordpressDialect;

pub struct WordpressFilter;

impl Filter for WordpressFilter {
    fn id(&self) -> &'static str {
        "wordpress"
    }
    fn name(&self) -> &'static str {
        "Wordpress XML export"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = WordpressDialect::new();
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
        let dialect = WordpressDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
