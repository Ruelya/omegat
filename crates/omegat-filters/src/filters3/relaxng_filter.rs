//! Java `RelaxNGFilter`.

use crate::xml_filter::{parse_to_file, write_xml, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::relaxng_dialect::RelaxNGDialect;

pub struct RelaxNGFilter;

impl Filter for RelaxNGFilter {
    fn id(&self) -> &'static str {
        "relaxng"
    }
    fn name(&self) -> &'static str {
        "RELAX NG"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.rng"]
    }
    fn file_supported(&self, path: &Path, _ctx: &FilterContext) -> bool {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return false;
        };
        raw.contains("grammar")
            && regex::Regex::new(r#"xmlns(:\w+)?="http://relaxng.org/ns/structure/1.0""#)
                .map(|re| re.is_match(&raw))
                .unwrap_or(false)
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = RelaxNGDialect::new();
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
        let dialect = RelaxNGDialect::new();
        let mut hooks = DefaultHooks::write(translations);
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
