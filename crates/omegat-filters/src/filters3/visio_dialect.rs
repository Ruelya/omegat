//! Java `VisioDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};

pub struct VisioDialect {
    inner: DefaultXmlDialect,
}

impl VisioDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["Text"]);
        inner.define_intact_tags(&[
            "DocumentProperties", "DocumentSettings", "Colors", "FaceNames", "StyleSheets",
            "DocumentSheet", "Masters", "Misc", "TextBlock", "Geom", "Para", "Char",
            "Connection", "XForm", "Line", "Fill", "Event", "PageSheet", "PageProps",
            "PageLayout", "PrintProps", "PageHeight", "PageWidth", "Image", "PinY",
            "Width", "Height", "XForm1D", "EndX", "LayerMem", "TextXForm", "Control",
            "ForeignData", "Foreign", "Menu", "Act", "User", "Help", "Copyright",
            "VBProjectData", "FooterMargin", "HeaderMargin", "HeaderFooter", "Window",
            "Windows", "EventList", "Scratch", "Protection", "Layout", "Icon",
            "vx:Event", "v14:Geom", "vx:Fill", "PreviewPicture", "vx:Char", "vx:Color",
            "vx:Line", "FillForegnd", "ShdwBkgnd", "TextBkgnd", "vx:TextBkgnd",
        ]);
        Self { inner }
    }
}

impl Default for VisioDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for VisioDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
