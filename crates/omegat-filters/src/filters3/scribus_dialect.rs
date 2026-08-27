//! Java `ScribusDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct ScribusDialect {
    inner: DefaultXmlDialect,
}

impl ScribusDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "SCRIBUSUTF8NEW");
        inner.define_translatable_attributes(&["CH"]);
        Self { inner }
    }
}

impl Default for ScribusDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for ScribusDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
