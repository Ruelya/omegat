//! filters3: one Java `*Filter` / `*Dialect` pair per file.

pub mod android_dialect;
pub mod android_filter;
pub mod camtasia_dialect;
pub mod camtasia_filter;
pub mod docbook_dialect;
pub mod docbook_filter;
pub mod flash_dialect;
pub mod flash_filter;
pub mod helpandmanual_dialect;
pub mod helpandmanual_filter;
pub mod infix_dialect;
pub mod infix_filter;
pub mod l10nmgr_dialect;
pub mod l10nmgr_filter;
pub mod opendoc_dialect;
pub mod opendoc_filter;
pub mod openxml_dialect;
pub mod openxml_filter;
pub mod properties_dialect;
pub mod properties_xml_filter;
pub mod relaxng_dialect;
pub mod relaxng_filter;
pub mod resx_dialect;
pub mod resx_filter;
pub mod schematron_dialect;
pub mod schematron_filter;
pub mod scribus_dialect;
pub mod scribus_filter;
pub mod svg_dialect;
pub mod svg_filter;
pub mod txml_dialect;
pub mod txml_filter;
pub mod typo3_dialect;
pub mod typo3_filter;
pub mod visio_dialect;
pub mod visio_filter;
pub mod wix_dialect;
pub mod wix_filter;
pub mod wordpress_dialect;
pub mod wordpress_filter;
pub mod xhtml_dialect;
pub mod xhtml_filter;
pub mod xliff_dialect;
pub mod xliff_filter;
pub mod xmlspreadsheet_dialect;
pub mod xmlspreadsheet_filter;

use crate::xml_dialect::{file_looks_like, ConstraintKind, DefaultXmlDialect, XmlDialect};
use std::collections::HashMap;

/// Snapshot of a dialect tag set, matching `dialect_tags.json` field names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialectTagSet {
    pub id: String,
    pub paragraph: Vec<String>,
    pub intact: Vec<String>,
    pub out_of_turn: Vec<String>,
    pub preformat: Vec<String>,
    pub attrs: Vec<String>,
    pub tag_attrs: Vec<(String, Vec<String>)>,
    pub constraints: Vec<(String, String)>,
}

fn sorted_set(set: &std::collections::HashSet<String>) -> Vec<String> {
    let mut v: Vec<String> = set.iter().cloned().collect();
    v.sort();
    v
}

fn constraint_name(kind: ConstraintKind) -> &'static str {
    match kind {
        ConstraintKind::Doctype => "doctype",
        ConstraintKind::PublicDoctype => "public_doctype",
        ConstraintKind::SystemDoctype => "system_doctype",
        ConstraintKind::Root => "root",
        ConstraintKind::Xmlns => "xmlns",
    }
}

pub fn snapshot_dialect(id: &str, dialect: &dyn XmlDialect) -> DialectTagSet {
    snapshot_base(id, dialect.base())
}

fn snapshot_base(id: &str, base: &DefaultXmlDialect) -> DialectTagSet {
    let mut tag_attrs: Vec<(String, Vec<String>)> = base
        .translatable_tag_attributes
        .iter()
        .map(|(k, v)| (k.clone(), sorted_set(v)))
        .collect();
    tag_attrs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut constraints: Vec<(String, String)> = base
        .constraints
        .iter()
        .map(|(k, v)| (constraint_name(*k).to_string(), v.clone()))
        .collect();
    constraints.sort_by(|a, b| a.0.cmp(&b.0));
    DialectTagSet {
        id: id.to_string(),
        paragraph: sorted_set(&base.paragraph_tags),
        intact: sorted_set(&base.intact_tags),
        out_of_turn: sorted_set(&base.out_of_turn_tags),
        preformat: sorted_set(&base.preformat_tags),
        attrs: sorted_set(&base.translatable_attributes),
        tag_attrs,
        constraints,
    }
}

/// All 23 filters3 dialects with empty options (same as `ExportGoldens.exportDialectTags`).
pub fn all_dialect_tag_sets() -> Vec<DialectTagSet> {
    let empty = HashMap::new();
    vec![
        snapshot_dialect("android", &android_dialect::AndroidDialect::new()),
        snapshot_dialect("camtasia", &camtasia_dialect::CamtasiaWindowsDialect::new()),
        snapshot_dialect("docbook", &docbook_dialect::DocBookDialect::new()),
        snapshot_dialect("flash", &flash_dialect::FlashDialect::new()),
        snapshot_dialect(
            "helpandmanual",
            &helpandmanual_dialect::HelpAndManualDialect::new(),
        ),
        snapshot_dialect("infix", &infix_dialect::InfixDialect::new()),
        snapshot_dialect("l10nmgr", &l10nmgr_dialect::L10nmgrDialect::new()),
        snapshot_dialect("opendoc", &opendoc_dialect::OpenDocDialect::new(&empty)),
        snapshot_dialect("openxml", &openxml_dialect::OpenXmlDialect::new(&empty)),
        snapshot_dialect("propxml", &properties_dialect::PropertiesDialect::new()),
        snapshot_dialect("relaxng", &relaxng_dialect::RelaxNGDialect::new()),
        snapshot_dialect("resx", &resx_dialect::ResXDialect::new()),
        snapshot_dialect("schematron", &schematron_dialect::SchematronDialect::new()),
        snapshot_dialect("scribus", &scribus_dialect::ScribusDialect::new()),
        snapshot_dialect("svg", &svg_dialect::SvgDialect::new()),
        snapshot_dialect("txml", &txml_dialect::TXMLDialect::new()),
        snapshot_dialect("typo3", &typo3_dialect::Typo3Dialect::new()),
        snapshot_dialect("visio", &visio_dialect::VisioDialect::new()),
        snapshot_dialect("wix", &wix_dialect::WiXDialect::new()),
        snapshot_dialect("wordpress", &wordpress_dialect::WordpressDialect::new()),
        snapshot_dialect("xhtml", &xhtml_dialect::XhtmlDialect::new(&empty)),
        snapshot_dialect("xliff", &xliff_dialect::XliffDialect::new(&empty)),
        snapshot_dialect(
            "xmlss",
            &xmlspreadsheet_dialect::XMLSpreadsheetDialect::new(),
        ),
    ]
}

/// Constrained XML dialects only. Unknown XML is not Android.
pub fn sniff_xml_id(raw: &str) -> Option<&'static str> {
    if file_looks_like(raw, &android_dialect::AndroidDialect::new()) {
        return Some("android");
    }
    if file_looks_like(raw, &properties_dialect::PropertiesDialect::new()) {
        return Some("propxml");
    }
    if file_looks_like(raw, &docbook_dialect::DocBookDialect::new()) {
        return Some("docbook");
    }
    if file_looks_like(raw, &helpandmanual_dialect::HelpAndManualDialect::new()) {
        return Some("helpandmanual");
    }
    if file_looks_like(raw, &typo3_dialect::Typo3Dialect::new()) {
        return Some("typo3");
    }
    if file_looks_like(raw, &l10nmgr_dialect::L10nmgrDialect::new()) {
        return Some("l10nmgr");
    }
    if file_looks_like(raw, &infix_dialect::InfixDialect::new()) {
        return Some("infix");
    }
    if file_looks_like(raw, &xmlspreadsheet_dialect::XMLSpreadsheetDialect::new()) {
        return Some("xmlss");
    }
    if flash_dialect::file_looks_like(raw) {
        return Some("flash");
    }
    if wordpress_dialect::file_looks_like(raw) {
        return Some("wordpress");
    }
    if file_looks_like(raw, &xhtml_dialect::XhtmlDialect::new(&Default::default())) {
        return Some("xhtml");
    }
    None
}
