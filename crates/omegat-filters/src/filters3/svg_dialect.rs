//! Java `SvgDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct SvgDialect {
    inner: DefaultXmlDialect,
}

impl SvgDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "svg");
        inner.define_paragraph_tags(&["svg", "text", "p", "flowRoot"]);
        inner.define_intact_tags(&["style", "image", "path", "dc:format"]);
        Self { inner }
    }
}

impl Default for SvgDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for SvgDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
