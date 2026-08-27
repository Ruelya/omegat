//! Java `TXMLDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

pub struct TXMLDialect {
    inner: DefaultXmlDialect,
}

impl TXMLDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["source", "target"]);
        inner.define_intact_tags(&["source", "ut", "skeleton", "revisions"]);
        Self { inner }
    }
}

impl Default for TXMLDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for TXMLDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
