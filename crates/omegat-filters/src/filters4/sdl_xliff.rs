//! Java `org.omegat.filters4.xml.xliff.SdlXliff`.

use super::abstract_xml::{parse_xml_file, write_xml_file};
use super::xliff1_filter::{Xliff1Filter, Xliff1Proc};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

const SDL_NS: &str = "http://sdl.com/FileTypes/SdlXliff/1.0";

pub struct SdlXliffFilter;

fn make_proc(translations: Option<&HashMap<String, String>>) -> Xliff1Proc {
    let mut p = match translations {
        Some(m) => Xliff1Proc::with_translations(m),
        None => Xliff1Proc::new(),
    };
    p.standard_state = false;
    p.event_on_cmt_defs = true;
    p
}

impl Filter for SdlXliffFilter {
    fn id(&self) -> &'static str {
        "sdlxliff"
    }
    fn name(&self) -> &'static str {
        "SDL XLIFF"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.sdlxliff"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let mut proc = make_proc(None);
        let segments = parse_xml_file(path, &mut proc)?;
        Ok(ParsedFile {
            segments,
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let mut proc = make_proc(Some(translations));
        write_xml_file(source_path, dest_path, &mut proc)?;
        Ok(())
    }
}

/// Java `SdlXliff.isFileSupported`: `sdl:version` or XLIFF 1.x.
pub fn looks_like_sdl_xliff(raw: &str) -> bool {
    raw.contains(SDL_NS) && raw.contains("version")
}

impl SdlXliffFilter {
    pub fn inner_xliff1() -> Xliff1Filter {
        Xliff1Filter
    }
}
