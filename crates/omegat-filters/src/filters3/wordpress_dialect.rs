//! Java `WordpressDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

const WORDPRESS_NAMESPACE_PREFIX: &str = "http://wordpress.org/export/";

pub struct WordpressDialect {
    inner: DefaultXmlDialect,
}

impl WordpressDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["channel", "content:encoded", "title", "description"]);
        inner.define_intact_tags(&[
            "pubDate",
            "generator",
            "dc:creator",
            "link",
            "guid",
            "title",
            "category",
        ]);
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

/// Java `WordpressDialect.WORDPRESS_XMLNS` content sniff over the read-ahead
/// buffer. WordPress appends its export format version after this prefix.
pub fn file_looks_like(raw: &str) -> bool {
    let limit = raw
        .char_indices()
        .nth(8192)
        .map(|(i, _)| i)
        .unwrap_or(raw.len());
    let raw = &raw[..limit];
    let Some(namespace_at) = raw.find(WORDPRESS_NAMESPACE_PREFIX) else {
        return false;
    };
    let before = &raw[..namespace_at];
    let Some(attribute_at) = before.rfind("xmlns") else {
        return false;
    };
    let attribute = &before[attribute_at..];
    let prefixed = attribute
        .strip_prefix("xmlns:")
        .and_then(|value| value.strip_suffix("=\""))
        .is_some_and(|prefix| {
            !prefix.is_empty()
                && prefix
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        });
    attribute == "xmlns=\"" || prefixed
}
