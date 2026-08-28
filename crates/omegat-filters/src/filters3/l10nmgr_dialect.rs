//! Java `L10nmgrDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct L10nmgrDialect {
    inner: DefaultXmlDialect,
}

impl L10nmgrDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "TYPO3L10N");
        inner.define_paragraph_tags(&[
            "pageGrp", "data", "title", "address", "blockquote", "center", "div",
            "h1", "h2", "h3", "h4", "h5", "table", "th", "tr", "td", "p", "ol", "ul",
            "li", "dl", "dt", "dd", "form", "textarea", "fieldset", "legend", "label",
            "select", "option", "hr",
        ]);
        inner.define_intact_tags(&["head"]);
        inner.closing_tag_required = true;
        Self { inner }
    }
}

impl Default for L10nmgrDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for L10nmgrDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
