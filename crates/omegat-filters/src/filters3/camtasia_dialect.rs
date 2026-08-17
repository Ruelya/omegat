//! Java `CamtasiaWindowsDialect`.

use crate::xml_dialect::{ConstraintKind, DefaultXmlDialect, XmlDialect};

pub struct CamtasiaWindowsDialect {
    inner: DefaultXmlDialect,
}

impl CamtasiaWindowsDialect {
    pub fn new() -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_constraint(ConstraintKind::Root, "Project_Data");
        inner.define_paragraph_tags(&[
            "Caption", "RichTextHTML", "strOverlayRichText", "Text", "TitleName",
            "RichText", "Value", "Project_Notes", "JumpURL",
        ]);
        inner.define_intact_tags(&[
            "Accel_cmd", "Accel_fVirt", "Action", "AddTextDropShadow", "AlwaysDisplay",
            "AudioClickReduction", "Duration", "End", "FadeIn", "FadeOut", "Height",
            "ID", "ImagePath", "Opacity", "Start", "Style", "Time", "Type", "UniqueID",
            "Width",
        ]);
        Self { inner }
    }
}

impl Default for CamtasiaWindowsDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDialect for CamtasiaWindowsDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
