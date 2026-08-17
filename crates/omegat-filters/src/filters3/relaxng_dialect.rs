//! Java `RelaxNGDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct RelaxNGDialect {
    inner: DefaultXmlDialect,
}

impl RelaxNGDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "grammar");
        inner.define_constraint(ConstraintKind::Xmlns, r"http://relaxng.org/ns/structure/1.0");
        inner.define_paragraph_tags(&["documentation", "a:documentation"]);
        inner.define_intact_tags(&["value", "name", "nsName"]);
        Self { inner }
    }
}

impl Default for RelaxNGDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for RelaxNGDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
