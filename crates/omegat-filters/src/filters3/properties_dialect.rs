//! Java `PropertiesDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct PropertiesDialect {
    inner: DefaultXmlDialect,
}

impl PropertiesDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "properties");
        inner.define_paragraph_tags(&["entry"]);
        Self { inner }
    }
}

impl Default for PropertiesDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for PropertiesDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter().any(|(n, v)| {
            n.eq_ignore_ascii_case("translate")
                && matches!(v.to_ascii_uppercase().as_str(), "FALSE" | "NO" | "0")
        })
    }
}
