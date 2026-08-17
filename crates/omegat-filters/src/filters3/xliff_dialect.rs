//! Java `XLIFFDialect`.

use crate::xml_dialect::{ContentTagType, DefaultXmlDialect, XmlDialect};
use crate::xml_engine::XmlTag;
use std::collections::HashMap;

pub struct XliffDialect {
    inner: DefaultXmlDialect,
    change_state_to_needs_review: bool,
}

impl XliffDialect {
    pub fn new(options: &HashMap<String, String>) -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["source", "target"]);
        inner.define_out_of_turn_tags(&["sub"]);
        let compat26 = flag(options, "compatibility26", false);
        if compat26 {
            inner.define_intact_tags(&[
                "source",
                "header",
                "bin-unit",
                "prop-group",
                "count-group",
                "alt-trans",
                "note",
                "ph",
                "bpt",
                "ept",
                "it",
                "context",
                "seg-source",
                "sdl:seg-defs",
            ]);
        } else {
            inner.define_intact_tags(&[
                "source",
                "header",
                "bin-unit",
                "prop-group",
                "count-group",
                "alt-trans",
                "note",
                "context",
                "seg-source",
                "sdl:seg-defs",
            ]);
            inner.define_content_based_tag("bpt", ContentTagType::Begin);
            inner.define_content_based_tag("ept", ContentTagType::End);
            inner.define_content_based_tag("it", ContentTagType::Alone);
            inner.define_content_based_tag("ph", ContentTagType::Alone);
        }
        Self {
            inner,
            change_state_to_needs_review: flag(
                options,
                "changetargetstateneedsreviewtranslation",
                false,
            ),
        }
    }
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl XmlDialect for XliffDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_preformat_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        tag.eq_ignore_ascii_case("mrk")
            && atts
                .iter()
                .any(|(n, v)| n.eq_ignore_ascii_case("mtype") && v.eq_ignore_ascii_case("seg"))
    }

    fn validate_intact_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        if tag.starts_with("str:") {
            return true;
        }
        if !tag.eq_ignore_ascii_case("group")
            && !tag.eq_ignore_ascii_case("trans-unit")
            && !tag.eq_ignore_ascii_case("bin-unit")
        {
            return false;
        }
        atts.iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("translate") && v.eq_ignore_ascii_case("no"))
    }

    fn validate_content_based_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        tag == "mrk"
            && atts
                .iter()
                .any(|(n, v)| n == "mtype" && v == "protected")
    }

    fn handle_xml_tag(&self, tag: &mut XmlTag, translated: bool) {
        if tag.tag != "target" {
            return;
        }
        if let Some(attr) = tag.attrs.iter_mut().find(|a| a.name == "state") {
            let next = if self.change_state_to_needs_review {
                "needs-review-translation"
            } else {
                "translated"
            };
            if translated
                && (attr.value == "needs-translation" || attr.value == "needs-review-translation")
            {
                attr.value = next.to_string();
            } else if attr.value == "new" {
                attr.value = if translated {
                    next.to_string()
                } else {
                    "needs-translation".into()
                };
            }
        }
    }
}
