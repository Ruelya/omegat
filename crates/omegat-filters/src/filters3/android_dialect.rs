//! Java `AndroidDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct AndroidDialect {
    inner: DefaultXmlDialect,
}

impl AndroidDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "resources");
        inner.define_paragraph_tags(&["string", "item"]);
        Self { inner }
    }
}

impl Default for AndroidDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for AndroidDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter().any(|(n, v)| {
            (n.eq_ignore_ascii_case("translatable") || n.eq_ignore_ascii_case("translate"))
                && v.eq_ignore_ascii_case("false")
        })
    }
}
