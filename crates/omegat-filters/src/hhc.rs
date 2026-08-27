//! Java `org.omegat.filters2.hhc.HHCFilter2` + `HHCFilterVisitor`.

use crate::html::{process_html, VisitorKind};
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct HhcFilter;

impl Filter for HhcFilter {
    fn id(&self) -> &'static str {
        "hhc"
    }
    fn name(&self) -> &'static str {
        "HTML Help Compiler"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.hhc", "*.hhk"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process_html(&read_to_string(path)?, ctx, VisitorKind::Hhc, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let out = process_html(
            &read_to_string(source_path)?,
            ctx,
            VisitorKind::Hhc,
            Some(translations),
        )
        .written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}
