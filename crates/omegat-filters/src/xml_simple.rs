//! filters3 XML dialects. Each filter has its own Java `*Dialect` table.

use crate::dialect::{self, XmlDialect};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

macro_rules! dialect_filter {
    ($ty:ident, $id:expr, $name:expr, $masks:expr, $phase:expr, $dialect:expr) => {
        pub struct $ty;
        impl Filter for $ty {
            fn id(&self) -> &'static str {
                $id
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn default_masks(&self) -> &'static [&'static str] {
                $masks
            }
            fn phase(&self) -> u8 {
                $phase
            }
            fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
                let _ = ctx;
                dialect::parse_dialect(&crate::read_to_string(path)?, $dialect)
            }
            fn write(
                &self,
                source_path: &Path,
                dest_path: &Path,
                translations: &HashMap<String, String>,
                ctx: &FilterContext,
            ) -> Result<()> {
                dialect::write_dialect(source_path, dest_path, translations, $dialect, ctx)
            }
        }
    };
}

dialect_filter!(AndroidFilter, "android", "Android Resources", &["*.xml"], 3, dialect::ANDROID);
dialect_filter!(XhtmlFilter, "xhtml", "XHTML", &["*.xhtml", "*.html"], 3, dialect::XHTML);
dialect_filter!(PropertiesXmlFilter, "propxml", "Java Properties XML", &["*.xml"], 3, dialect::PROPXML);
dialect_filter!(ResxFilter, "resx", "ResX", &["*.resx"], 3, dialect::RESX);
dialect_filter!(WixFilter, "wix", "WiX Localization", &["*.wxl"], 3, dialect::WIX);
dialect_filter!(SvgFilter, "svg", "SVG", &["*.svg"], 3, dialect::SVG);
dialect_filter!(HelpAndManualFilter, "helpandmanual", "Help & Manual", &["*.xml"], 3, dialect::HELPANDMANUAL);
dialect_filter!(SchematronFilter, "schematron", "Schematron", &["*.sch"], 3, dialect::SCHEMATRON);
dialect_filter!(RelaxNgFilter, "relaxng", "RELAX NG", &["*.rng"], 3, dialect::RELAXNG);
dialect_filter!(CamtasiaFilter, "camtasia", "Camtasia for Windows", &["*.camproj"], 3, dialect::CAMTASIA);
dialect_filter!(Typo3Filter, "typo3", "Typo3 LocManager", &["*.xml"], 3, dialect::TYPO3);
dialect_filter!(L10nMgrFilter, "l10nmgr", "Typo3 l10nmgr", &["*.xml"], 3, dialect::L10NMGR);
dialect_filter!(InfixFilter, "infix", "Infix", &["*.xml"], 3, dialect::INFIX);
dialect_filter!(FlashFilter, "flash", "Flash XML Export", &["*.xml"], 3, dialect::FLASH);
dialect_filter!(TxmlFilter, "txml", "Wordfast TXML", &["*.txml"], 3, dialect::TXML);
dialect_filter!(WordpressFilter, "wordpress", "Wordpress XML export", &["*.xml"], 3, dialect::WORDPRESS);
dialect_filter!(ScribusFilter, "scribus", "Scribus", &["*.sla"], 3, dialect::SCRIBUS);
dialect_filter!(XmlSpreadsheetFilter, "xmlss", "XML Spreadsheet 2003", &["*.xml"], 3, dialect::XMLSS);
dialect_filter!(DocBookFilter, "docbook", "DocBook", &["*.xml", "*.dbk"], 4, dialect::DOCBOOK);
dialect_filter!(VisioFilter, "visio", "Visio", &["*.vdx", "*.vsdx"], 4, dialect::VISIO);

#[allow(dead_code)]
pub fn dialect_for(id: &str) -> Option<XmlDialect> {
    Some(match id {
        "android" => dialect::ANDROID,
        "xhtml" => dialect::XHTML,
        "propxml" => dialect::PROPXML,
        "resx" => dialect::RESX,
        "wix" => dialect::WIX,
        "svg" => dialect::SVG,
        "helpandmanual" => dialect::HELPANDMANUAL,
        "schematron" => dialect::SCHEMATRON,
        "relaxng" => dialect::RELAXNG,
        "camtasia" => dialect::CAMTASIA,
        "typo3" => dialect::TYPO3,
        "l10nmgr" => dialect::L10NMGR,
        "infix" => dialect::INFIX,
        "flash" => dialect::FLASH,
        "txml" => dialect::TXML,
        "wordpress" => dialect::WORDPRESS,
        "scribus" => dialect::SCRIBUS,
        "xmlss" => dialect::XMLSS,
        "docbook" => dialect::DOCBOOK,
        "visio" => dialect::VISIO,
        _ => return None,
    })
}
