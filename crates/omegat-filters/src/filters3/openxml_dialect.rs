//! Java `OpenXMLDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};
use std::collections::HashMap;

pub struct OpenXmlDialect {
    inner: DefaultXmlDialect,
}

impl OpenXmlDialect {
    pub fn new(options: &HashMap<String, String>) -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&[
            "w:p",
            "w:tab",
            "dc:title",
            "dc:subject",
            "dc:creator",
            "si",
            "comment",
            "definedName",
            "a:p",
            "c:v",
            "Text",
        ]);
        if flag(options, "breakOnBr", true) {
            inner.define_paragraph_tags(&["w:br"]);
        }
        if flag(options, "translateHiddenText", false) {
            inner.define_out_of_turn_tag("w:instrText");
        } else {
            inner.define_intact_tag("w:instrText");
        }
        if !flag(options, "translateFallbackText", false) {
            inner.define_intact_tag("mc:Fallback");
        }
        inner.define_intact_tags(&[
            "authors",
            "rPh",
            "definedNames",
            "p:attrName",
            "a:tableStyleId",
            "c:f",
            "c:formatCode",
            "wp:align",
            "wp:posOffset",
            "wp14:pctWidth",
            "wp14:pctHeight",
            "w:fldChar",
            "cp:lastModifiedBy",
            "cp:revision",
            "cp:lastPrinted",
            "dcterms:created",
            "dcterms:modified",
            "cp:version",
            "xdr:col",
            "xdr:row",
            "xdr:colOff",
            "xdr:rowOff",
            "DocumentProperties",
            "DocumentSettings",
            "Colors",
            "FaceNames",
            "StyleSheets",
            "DocumentSheet",
            "Masters",
            "Misc",
            "TextBlock",
            "Geom",
            "Para",
            "Char",
            "Connection",
            "XForm",
            "Line",
            "Fill",
            "Event",
            "PageSheet",
            "PageProps",
            "PageLayout",
            "PrintProps",
            "PageHeight",
            "PageWidth",
            "Image",
            "PinY",
            "Width",
            "Height",
            "XForm1D",
            "EndX",
            "LayerMem",
            "TextXForm",
            "Control",
            "ForeignData",
            "Foreign",
            "Menu",
            "Act",
            "User",
            "Help",
            "Copyright",
            "VBProjectData",
            "FooterMargin",
            "HeaderMargin",
            "HeaderFooter",
            "Window",
            "Windows",
            "EventList",
            "Scratch",
            "Protection",
            "Layout",
            "Icon",
            "vx:Event",
            "v14:Geom",
            "vx:Fill",
            "PreviewPicture",
            "vx:Char",
            "vx:Color",
            "vx:Line",
            "FillForegnd",
            "ShdwBkgnd",
            "TextBkgnd",
            "vx:TextBkgnd",
        ]);
        inner.define_translatable_tag_attribute("sheet", "name");
        if flag(options, "translateWordArt", false) {
            inner.define_translatable_tag_attribute("v:textpath", "string");
        }
        if flag(options, "translateSlideLinks", false) {
            inner.define_translatable_tag_attribute("Relationship", "Target");
        }
        inner.tags_aggregation_enabled = flag(options, "aggregateTags", true);
        inner.force_space_preserving = flag(options, "preserveSpaces", true);
        Self { inner }
    }
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl XmlDialect for OpenXmlDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn validate_translatable_tag_attribute(
        &self,
        tag: &str,
        attribute: &str,
        atts: &[(String, String)],
    ) -> bool {
        if tag.eq_ignore_ascii_case("Relationship") && attribute.eq_ignore_ascii_case("Target") {
            return atts.iter().any(|(n, v)| {
                n.eq_ignore_ascii_case("TargetMode") && v.eq_ignore_ascii_case("External")
            });
        }
        true
    }
}
