//! Java `WordpressDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

pub struct WordpressDialect {
    inner: DefaultXmlDialect,
}

impl WordpressDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["channel", "content:encoded", "title", "description"]);
        inner.define_intact_tags(&["pubDate", "generator", "dc:creator", "link", "guid", "title", "category"]);
        Self { inner }
    }
}

impl Default for WordpressDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for WordpressDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, tag: &str, _atts: &[(String, String)]) -> bool {
        tag.starts_with("wp:")
    }
}
