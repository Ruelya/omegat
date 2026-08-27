//! Java `FlashDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

const FLASH_NAMESPACE: &str = "http://ns.adobe.com/xfl/2008/";

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

/// Java `FlashDialect.FLASH_XMLNS` content sniff over the read-ahead buffer.
pub fn file_looks_like(raw: &str) -> bool {
    let limit = raw
        .char_indices()
        .nth(8192)
        .map(|(i, _)| i)
        .unwrap_or(raw.len());
    let raw = &raw[..limit];
    let Some(namespace_at) = raw.find(FLASH_NAMESPACE) else {
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
    (attribute == "xmlns=\"" || prefixed)
        && raw[namespace_at + FLASH_NAMESPACE.len()..].starts_with('"')
}
