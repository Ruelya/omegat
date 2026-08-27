//! Java `OpenDocDialect`.

use crate::xml_dialect::{DefaultXmlDialect, XmlDialect};
use std::collections::HashMap;

pub struct OpenDocDialect {
    inner: DefaultXmlDialect,
}

impl OpenDocDialect {
    pub fn new(options: &HashMap<String, String>) -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_shortcuts(&[
            "text:line-break",
            "br",
            "text:a",
            "a",
            "text:span",
            "f",
            "text:s",
            "s",
            "text:alphabetical-index-mark",
            "i",
            "text:alphabetical-index-mark-start",
            "is",
            "text:alphabetical-index-mark-end",
            "ie",
            "text:tab-stop",
            "t",
            "text:user-defined",
            "ud",
            "text:sequence",
            "seq",
            "draw:image",
            "di",
            "draw:frame",
            "df",
            "draw:object-ole",
            "do",
            "text:bookmark",
            "bk",
            "text:bookmark-start",
            "bs",
            "text:bookmark-end",
            "be",
            "text:bookmark-ref",
            "bf",
            "text:reference-mark",
            "rm",
            "text:reference-mark-start",
            "rs",
            "text:reference-mark-end",
            "re",
            "text:reference-ref",
            "rf",
            "text:change",
            "tc",
            "text:change-start",
            "ts",
            "text:change-end",
            "te",
            "dc:creator",
            "dc",
            "dc:date",
            "dd",
            "text:note-citation",
            "nc",
            "text:note-body",
            "nb",
        ]);
        inner.define_paragraph_tags(&[
            "text:p",
            "text:h",
            "dc:title",
            "dc:description",
            "dc:subject",
            "meta:keyword",
            "dc:language",
            "meta:user-defined",
            "text:tab",
        ]);
        inner.define_intact_tags(&[
            "text:note-citation",
            "text:change",
            "text:tracked-changes",
            "office:scripts",
            "office:font-face-decls",
            "office:automatic-styles",
            "office:styles",
            "meta:generator",
            "meta:initial-creator",
            "meta:creation-date",
            "meta:print-date",
            "dc:creator",
            "dc:date",
            "dc:language",
            "meta:editing-cycles",
            "meta:editing-duration",
            "meta:user-defined",
        ]);
        if flag(options, "translateNotes", true) {
            inner.define_out_of_turn_tag("text:note");
            inner.define_out_of_turn_tag("text:footnote");
        } else {
            inner.define_intact_tag("text:note");
            inner.define_intact_tag("text:footnote");
        }
        if flag(options, "translateComments", true) {
            inner.define_out_of_turn_tag("office:annotation");
        } else {
            inner.define_intact_tag("office:annotation");
        }
        if flag(options, "translateIndexes", true) {
            inner.define_translatable_tag_attributes(
                "text:alphabetical-index-mark",
                &["text:string-value", "text:key1", "text:key2"],
            );
        }
        if flag(options, "translateBookmarks", false) {
            inner.define_translatable_tags_attribute(
                &["text:bookmark", "text:bookmark-start", "text:bookmark-end"],
                "text:name",
            );
            inner.define_translatable_tag_attribute("text:bookmark-ref", "text:ref-name");
        }
        if !flag(options, "translateBookmarkRefs", true) {
            inner.define_intact_tag("text:bookmark-ref");
        }
        if !flag(options, "translatePresNotes", true) {
            inner.define_intact_tag("presentation:notes");
        }
        if flag(options, "translateLinks", false) {
            inner.define_translatable_attribute("xlink:href");
        }
        if flag(options, "translateSheetNames", false) {
            inner.define_translatable_tag_attribute("table:table", "table:name");
        }
        Self { inner }
    }
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl XmlDialect for OpenDocDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }
}
