//! Java `WiXDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct WiXDialect {
    inner: DefaultXmlDialect,
}

impl WiXDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "WixLocalization");
        inner.define_paragraph_tags(&["String"]);
        Self { inner }
    }
}

impl Default for WiXDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for WiXDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
