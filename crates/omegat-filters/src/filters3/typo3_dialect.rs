//! Java `Typo3Dialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct Typo3Dialect {
    inner: DefaultXmlDialect,
}

impl Typo3Dialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "t3_tt_content");
        inner.define_paragraph_tags(&[
            "title", "subtitle", "p", "br", "header", "li", "td", "abstract",
            "image_link", "imagecaption",
        ]);
        inner.define_intact_tags(&["l18n_diffsource"]);
        inner.closing_tag_required = true;
        Self { inner }
    }
}

impl Default for Typo3Dialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for Typo3Dialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_translatable_tag(&self, _tag: &str, atts: &[(String, String)]) -> bool {
        atts.iter().any(|(n, v)| n.eq_ignore_ascii_case("localizable") && v == "1")
    }
}
