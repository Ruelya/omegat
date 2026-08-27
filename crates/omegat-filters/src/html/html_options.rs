//! Java `org.omegat.filters2.html2.HTMLOptions`.

use crate::FilterContext;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteMode {
    Always,
    IfHeader,
    IfMeta,
    Never,
}

impl RewriteMode {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "ALWAYS" => Self::Always,
            "IFMETA" => Self::IfMeta,
            "NEVER" => Self::Never,
            _ => Self::IfHeader,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HtmlOptions {
    pub rewrite_encoding: RewriteMode,
    pub translate_href: bool,
    pub translate_src: bool,
    pub translate_lang: bool,
    pub translate_hreflang: bool,
    pub translate_value: bool,
    pub translate_button_value: bool,
    pub paragraph_on_br: bool,
    pub skip_regexp: String,
    pub skip_meta: String,
    pub ignore_tags: String,
    pub remove_comments: bool,
    pub compress_whitespace: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            rewrite_encoding: RewriteMode::IfHeader,
            translate_href: true,
            translate_src: true,
            translate_lang: true,
            translate_hreflang: true,
            translate_value: true,
            translate_button_value: true,
            paragraph_on_br: false,
            skip_regexp: String::new(),
            skip_meta: "http-equiv=refresh,name=robots,name=revisit-after,http-equiv=expires,http-equiv=content-style-type,http-equiv=content-script-type".into(),
            ignore_tags: String::new(),
            remove_comments: false,
            compress_whitespace: false,
        }
    }
}

fn flag(map: &HashMap<String, String>, key: &str, default: bool) -> bool {
    map.get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl HtmlOptions {
    pub fn from_map(map: &HashMap<String, String>) -> Self {
        let mut o = Self::default();
        if let Some(v) = map.get("rewriteEncoding") {
            o.rewrite_encoding = RewriteMode::parse(v);
        }
        o.translate_href = flag(map, "translateHref", true);
        o.translate_src = flag(map, "translateSrc", true);
        o.translate_lang = flag(map, "translateLang", true);
        o.translate_hreflang = flag(map, "translateHreflang", true);
        o.translate_value = flag(map, "translateValue", true);
        o.translate_button_value = flag(map, "translateButtonValue", true);
        o.paragraph_on_br = flag(map, "paragraphOnBr", false);
        o.skip_regexp = map.get("skipRegExp").cloned().unwrap_or_default();
        if let Some(v) = map.get("skipMeta") {
            o.skip_meta = v.clone();
        }
        o.ignore_tags = map.get("ignoreTags").cloned().unwrap_or_default();
        o.remove_comments = flag(map, "removeComments", false);
        o.compress_whitespace = flag(map, "compressWhitespace", false);
        o
    }

    pub fn from_ctx(ctx: &FilterContext) -> Self {
        Self::from_map(&ctx.options)
    }

    pub fn skip_meta_pairs(&self) -> Vec<(String, String)> {
        parse_pairs(&self.skip_meta)
    }

    pub fn ignore_tag_pairs(&self) -> Vec<(String, String)> {
        parse_pairs(&self.ignore_tags)
    }
}

fn parse_pairs(s: &str) -> Vec<(String, String)> {
    s.split(',')
        .filter_map(|part| {
            let (k, v) = part.split_once('=')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase()))
        })
        .collect()
}
