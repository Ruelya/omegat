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

use crate::xml_dialect::file_looks_like;

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
    if file_looks_like(raw, &xhtml_dialect::XhtmlDialect::new(&Default::default())) {
        return Some("xhtml");
    }
    None
}
