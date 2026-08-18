//! Java `ResXDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

pub struct ResXDialect {
    inner: DefaultXmlDialect,
}

impl ResXDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["value"]);
        inner.define_intact_tags(&["resheader", "metadata", "comment"]);
        Self { inner }
    }
}

impl Default for ResXDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for ResXDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        if !tag.eq_ignore_ascii_case("data") {
            return false;
        }
        atts.iter().any(|(n, v)| {
            n.eq_ignore_ascii_case("type")
                || n.eq_ignore_ascii_case("mimetype")
                || (n.eq_ignore_ascii_case("name")
                    && (v.starts_with("&gt;") || v.ends_with("FieldName")))
        })
    }
}
