//! Java `SchematronDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct SchematronDialect {
    inner: DefaultXmlDialect,
}

impl SchematronDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "schema|pattern");
        inner.define_paragraph_tags(&["assert", "report"]);
        inner.define_intact_tags(&["phase", "active", "ns", "include", "key", "let"]);
        Self { inner }
    }
}

impl Default for SchematronDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for SchematronDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("translate") && v.eq_ignore_ascii_case("false"))
    }
}
