//! Java `XMLSpreadsheetDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct XMLSpreadsheetDialect {
    inner: DefaultXmlDialect,
}

impl XMLSpreadsheetDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "Workbook");
        inner.define_paragraph_tags(&["Workbook", "Cell"]);
        inner.define_intact_tags(&[
            "DocumentProperties",
            "ExcelWorkbook",
            "WorksheetOptions",
            "OfficeDocumentSettings",
        ]);
        Self { inner }
    }
}

impl Default for XMLSpreadsheetDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for XMLSpreadsheetDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_intact_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        if !tag.eq_ignore_ascii_case("Data") {
            return false;
        }
        atts.iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("ss:type") && v.eq_ignore_ascii_case("number"))
    }
}
