//! Java `InfixDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct InfixDialect {
    inner: DefaultXmlDialect,
}

impl InfixDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "DOC");
        inner.define_paragraph_tags(&["STORY", "P"]);
        inner.define_shortcut("BR", "br");
        Self { inner }
    }
}

impl Default for InfixDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for InfixDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
