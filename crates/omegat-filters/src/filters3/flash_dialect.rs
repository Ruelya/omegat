//! Java `FlashDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

pub struct FlashDialect {
    inner: DefaultXmlDialect,
}

impl FlashDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["characters"]);
        inner.define_intact_tags(&["script"]);
        Self { inner }
    }
}

impl Default for FlashDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for FlashDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
