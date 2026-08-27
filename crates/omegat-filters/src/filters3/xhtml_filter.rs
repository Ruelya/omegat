//! Java `XHTMLFilter`.

use crate::xml_engine::FilterHooks;
use crate::xml_filter::{engine_config, parse_xml_cfg, write_xml_cfg, DefaultHooks};
use crate::{Filter, FilterContext, ParsedFile, ProtectedPart, Result};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::path::Path;

use super::xhtml_dialect::XhtmlDialect;

pub struct XhtmlFilter;

struct XhtmlHooks {
    inner: DefaultHooks,
    skip_regexp: Option<Regex>,
}

impl XhtmlHooks {
    fn parse(options: &HashMap<String, String>) -> Self {
        Self {
            inner: DefaultHooks::parse(),
            skip_regexp: compile_skip_regexp(options),
        }
    }

    fn write(options: &HashMap<String, String>, translations: &HashMap<String, String>) -> Self {
        Self {
            inner: DefaultHooks::write(translations),
            skip_regexp: compile_skip_regexp(options),
        }
    }
}

fn compile_skip_regexp(options: &HashMap<String, String>) -> Option<Regex> {
    let expression = options.get("skipRegExp")?.trim();
    if expression.is_empty() {
        return None;
    }
    // Java uses `Pattern.CASE_INSENSITIVE` and `Matcher.matches()`. An invalid
    // expression is logged and ignored there, so a Rust compile failure also
    // disables the optional matcher instead of rejecting the document.
    RegexBuilder::new(&format!(r"\A(?:{expression})\z"))
        .case_insensitive(true)
        .build()
        .ok()
}

impl FilterHooks for XhtmlHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]) {
        self.inner.tag_start(path, attrs);
    }

    fn tag_end(&mut self, path: &str) {
        self.inner.tag_end(path);
    }

    fn comment(&mut self, comment: &str) {
        self.inner.comment(comment);
    }

    fn text(&mut self, text: &str) {
        self.inner.text(text);
    }

    fn is_in_ignored(&self) -> bool {
        self.inner.is_in_ignored()
    }

    fn translate(&mut self, entry: &str, protected: &[ProtectedPart]) -> String {
        if self
            .skip_regexp
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(entry))
        {
            entry.to_string()
        } else {
            self.inner.translate(entry, protected)
        }
    }
}

impl Filter for XhtmlFilter {
    fn id(&self) -> &'static str {
        "xhtml"
    }
    fn name(&self) -> &'static str {
        "XHTML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xhtml", "*.html"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = XhtmlDialect::new(&ctx.options);
        let mut hooks = XhtmlHooks::parse(&ctx.options);
        let skeleton = parse_xml_cfg(path, &dialect, &mut hooks, engine_config(ctx))?;
        Ok(ParsedFile {
            segments: std::mem::take(&mut hooks.inner.segments),
            skeleton: Some(skeleton),
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = XhtmlDialect::new(&ctx.options);
        let mut hooks = XhtmlHooks::write(&ctx.options, translations);
        write_xml_cfg(
            source_path,
            dest_path,
            &dialect,
            &mut hooks,
            engine_config(ctx),
        )
    }
}
