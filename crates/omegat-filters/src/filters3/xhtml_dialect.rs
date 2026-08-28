//! Java `XHTMLDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};
use std::collections::HashMap;

pub struct XhtmlDialect {
    inner: DefaultXmlDialect,
    translate_value: bool,
    translate_button_value: bool,
    skip_meta: HashMap<String, ()>,
    ignore_tags: HashMap<String, ()>,
}

impl XhtmlDialect {
    pub fn new(options: &HashMap<String, String>) -> Self {
        let mut inner = DefaultXmlDialect::new();
        let ignore_doctype = options
            .get("ignoreDoctype")
            .map(|s| s == "true")
            .unwrap_or(false);
        if !ignore_doctype {
            inner.define_constraint(ConstraintKind::PublicDoctype, r"-//W3C//DTD XHTML.*");
        }
        inner.define_paragraph_tags(&[
            "html", "head", "title", "body", "address", "blockquote", "center", "div", "h1", "h2",
            "h3", "h4", "h5", "table", "th", "tr", "td", "p", "ol", "ul", "li", "dl", "dt", "dd",
            "form", "textarea", "fieldset", "legend", "label", "select", "option", "hr",
        ]);
        if flag(options, "paragraphOnBr", false) {
            inner.define_paragraph_tags(&["br"]);
        }
        inner.define_shortcut("br", "br");
        inner.define_preformat_tags(&["textarea", "pre"]);
        inner.define_intact_tags(&["style", "script", "object", "embed"]);
        inner.define_translatable_attributes(&[
            "abbr", "alt", "content", "dir", "summary", "title", "placeholder",
        ]);
        if flag(options, "translateHref", true) {
            inner.define_translatable_attribute("href");
        }
        if flag(options, "translateSrc", true) {
            inner.define_translatable_tag_attribute("img", "src");
        }
        if flag(options, "translateLang", true) {
            inner.define_translatable_attributes(&["lang", "xml:lang"]);
        }
        if flag(options, "translateHreflang", true) {
            inner.define_translatable_attribute("hreflang");
        }
        let translate_value = flag(options, "translateValue", true);
        let translate_button_value = flag(options, "translateButtonValue", true);
        if translate_value || translate_button_value {
            inner.define_translatable_tag_attribute("input", "value");
        }
        let skip_meta_str = options.get("skipMeta").cloned().unwrap_or_else(|| {
            "http-equiv=Content-Type,http-equiv=refresh,name=robots,name=revisit-after,http-equiv=expires,http-equiv=content-style-type,http-equiv=content-script-type".into()
        });
        let mut skip_meta = HashMap::new();
        for s in skip_meta_str.split(',') {
            skip_meta.insert(s.trim().to_ascii_uppercase(), ());
        }
        let mut ignore_tags = HashMap::new();
        if let Some(ig) = options.get("ignoreTags") {
            for s in ig.split(',') {
                ignore_tags.insert(s.trim().to_ascii_uppercase(), ());
            }
        }
        Self {
            inner,
            translate_value,
            translate_button_value,
            skip_meta,
            ignore_tags,
        }
    }
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl XmlDialect for XhtmlDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_translatable_tag_attribute(
        &self,
        tag: &str,
        attribute: &str,
        atts: &[(String, String)],
    ) -> bool {
        if tag.eq_ignore_ascii_case("INPUT") && attribute.eq_ignore_ascii_case("value") {
            if self.translate_value {
                return true;
            }
            if self.translate_button_value {
                return atts.iter().any(|(n, v)| {
                    n.eq_ignore_ascii_case("type")
                        && matches!(
                            v.to_ascii_lowercase().as_str(),
                            "button" | "submit" | "reset"
                        )
                });
            }
            return false;
        }
        if tag.eq_ignore_ascii_case("META") && attribute.eq_ignore_ascii_case("content") {
            for (n, v) in atts {
                let key = format!("{}={}", n.to_ascii_uppercase(), v.to_ascii_uppercase());
                if self.skip_meta.contains_key(&key) {
                    return false;
                }
            }
        }
        true
    }

    fn validate_intact_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter().any(|(n, v)| {
            let key = format!("{}={}", n.to_ascii_uppercase(), v.to_ascii_uppercase());
            self.ignore_tags.contains_key(&key)
        })
    }
}
