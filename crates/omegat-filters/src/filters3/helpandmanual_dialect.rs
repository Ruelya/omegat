//! Java `HelpAndManualDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct HelpAndManualDialect {
    inner: DefaultXmlDialect,
}

impl HelpAndManualDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "topic|map|helpproject");
        inner.define_paragraph_tags(&["caption", "config-value", "variable", "para", "title", "keyword", "li"]);
        inner.define_shortcut("link", "li");
        Self { inner }
    }
}

impl Default for HelpAndManualDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for HelpAndManualDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter().any(|(n, v)| {
            n.eq_ignore_ascii_case("translate")
                && matches!(v.to_ascii_lowercase().as_str(), "false" | "no" | "0")
        })
    }
}
