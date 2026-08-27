//! Java `XLIFFDialect`.

use crate::inline_tag::InlineTagHandler;
use crate::xml_dialect::{ContentTagType, DefaultXmlDialect, XmlDialect};
use crate::xml_engine::{Attr, Element, TagType, XmlTag};
use crate::ProtectedPart;
use std::collections::HashMap;

/// Java `XLIFFFilterTest#testHandleXMLTag` state transitions.
pub fn target_state_after(from: &str, translated: bool, review: bool) -> String {
    let mut options = HashMap::new();
    if review {
        options.insert(
            "changetargetstateneedsreviewtranslation".into(),
            "true".into(),
        );
    }
    let dialect = XliffDialect::new(&options);
    let mut tag = XmlTag {
        tag: "target".into(),
        shortcut: "t".into(),
        typ: TagType::Begin,
        attrs: vec![Attr {
            name: "state".into(),
            value: from.to_string(),
        }],
        start_attrs: vec![],
        index: 0,
    };
    dialect.handle_xml_tag(&mut tag, translated);
    tag.attrs
        .iter()
        .find(|a| a.name == "state")
        .map(|a| a.value.clone())
        .unwrap_or_default()
}

pub struct XliffDialect {
    inner: DefaultXmlDialect,
    change_state_to_needs_review: bool,
    force_shortcut_to_f: bool,
    ignore_type_for_ph: bool,
    ignore_type_for_bpt: bool,
}

impl XliffDialect {
    pub fn new(options: &HashMap<String, String>) -> Self {
        let mut inner = DefaultXmlDialect::new();
        inner.define_paragraph_tags(&["source", "target"]);
        inner.define_out_of_turn_tags(&["sub"]);
        let compat26 = flag(options, "compatibility26", false);
        if compat26 {
            inner.define_intact_tags(&[
                "source",
                "header",
                "bin-unit",
                "prop-group",
                "count-group",
                "alt-trans",
                "note",
                "ph",
                "bpt",
                "ept",
                "it",
                "context",
                "seg-source",
                "sdl:seg-defs",
            ]);
        } else {
            inner.define_intact_tags(&[
                "source",
                "header",
                "bin-unit",
                "prop-group",
                "count-group",
                "alt-trans",
                "note",
                "context",
                "seg-source",
                "sdl:seg-defs",
            ]);
            inner.define_content_based_tag("bpt", ContentTagType::Begin);
            inner.define_content_based_tag("ept", ContentTagType::End);
            inner.define_content_based_tag("it", ContentTagType::Alone);
            inner.define_content_based_tag("ph", ContentTagType::Alone);
        }
        Self {
            inner,
            change_state_to_needs_review: flag(
                options,
                "changetargetstateneedsreviewtranslation",
                false,
            ),
            force_shortcut_to_f: flag(options, "forceshortcut2f", false),
            ignore_type_for_ph: flag(options, "ignoretype4phtags", false),
            ignore_type_for_bpt: flag(options, "ignoretype4bpttags", false),
        }
    }
}

impl XliffDialect {
    fn content_based_shortcut(
        &self,
        tag: &XmlTag,
        inner: &[Element],
        handler: &mut InlineTagHandler,
    ) -> (String, i32, i32) {
        let attr = |name: &str| tag.attrs.iter().find(|a| a.name == name).map(|a| a.value.clone());
        match tag.tag.as_str() {
            "bpt" => {
                handler.start_bpt(&[attr("rid"), attr("id"), attr("i")]);
                let letter = self.calc_letter(tag, inner, self.ignore_type_for_bpt);
                handler.set_tag_shortcut_letter(letter);
                let idx = handler.end_bpt();
                let ch = letter_char(letter);
                (format!("<{ch}{idx}>"), letter, idx)
            }
            "ept" => {
                handler.start_ept(&[attr("rid"), attr("id"), attr("i")]);
                let idx = handler.end_ept();
                let letter = handler.get_tag_shortcut_letter();
                let ch = letter_char(letter);
                (format!("</{ch}{idx}>"), letter, idx)
            }
            "it" => {
                handler.start_other();
                handler.set_current_pos(attr("pos"));
                let idx = handler.end_other();
                let mut letter = self.calc_letter(tag, inner, false);
                let pos = handler.current_pos().unwrap_or("");
                if pos == "close" || pos == "end" {
                    if self.force_shortcut_to_f {
                        letter = b'f' as i32;
                    }
                    let ch = letter_char(letter);
                    (format!("</{ch}{idx}>"), letter, idx)
                } else {
                    let ch = letter_char(letter);
                    (format!("<{ch}{idx}>"), letter, idx)
                }
            }
            "ph" => {
                handler.start_other();
                let idx = handler.end_other();
                let letter = self.calc_letter(tag, inner, self.ignore_type_for_ph);
                let ch = letter_char(letter);
                (format!("<{ch}{idx}/>"), letter, idx)
            }
            "mrk" => {
                handler.start_other();
                let idx = handler.end_other();
                let inner_orig: String = inner.iter().map(|e| e.to_original()).collect();
                (
                    format!("<m{idx}>{inner_orig}</m{idx}>"),
                    b'm' as i32,
                    idx,
                )
            }
            _ => {
                handler.start_other();
                let idx = handler.end_other();
                (tag.to_shortcut(), 0, idx)
            }
        }
    }

    fn calc_letter(&self, tag: &XmlTag, inner: &[Element], ignore_type: bool) -> i32 {
        if let Some(Element::Text { text, .. }) = inner.first() {
            if let Some(c) = text.chars().find(|c| c.is_alphabetic()) {
                return c.to_ascii_lowercase() as i32;
            }
        }
        if !ignore_type {
            if let Some(typ) = tag
                .attrs
                .iter()
                .find(|a| a.name == "ctype" || a.name == "type")
                .map(|a| a.value.as_str())
            {
                if let Some(c) = typ.chars().find(|c| c.is_alphabetic()) {
                    return c.to_ascii_lowercase() as i32;
                }
            }
        }
        0
    }
}

fn letter_char(letter: i32) -> char {
    if letter == 0 {
        'f'
    } else {
        char::from_u32(letter as u32).unwrap_or('f')
    }
}

fn flag(options: &HashMap<String, String>, key: &str, default: bool) -> bool {
    options
        .get(key)
        .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(default)
}

impl XmlDialect for XliffDialect {
    fn base(&self) -> &DefaultXmlDialect {
        &self.inner
    }

    fn construct_shortcuts(
        &self,
        elements: &[Element],
        protected: &mut Vec<ProtectedPart>,
    ) -> String {
        protected.clear();
        let mut tag_handler = InlineTagHandler::new();
        let mut r = String::new();
        for el in elements {
            match el {
                Element::Intact {
                    tag,
                    inner,
                    content_based: true,
                } => {
                    let (shortcut, letter, idx) =
                        self.content_based_shortcut(tag, inner, &mut tag_handler);
                    let _ = (letter, idx);
                    r.push_str(&shortcut);
                    protected.push(ProtectedPart {
                        text: shortcut,
                        details: el.to_original(),
                    });
                }
                Element::Tag(tag) => {
                    let idx = tag_handler.paired(&tag.tag, tag.typ);
                    let mut t = tag.clone();
                    t.index = idx;
                    let shortcut = t.to_shortcut();
                    r.push_str(&shortcut);
                    protected.push(ProtectedPart {
                        text: shortcut,
                        details: el.to_original(),
                    });
                }
                _ => r.push_str(&el.to_shortcut()),
            }
        }
        r
    }

    fn validate_preformat_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        tag.eq_ignore_ascii_case("mrk")
            && atts
                .iter()
                .any(|(n, v)| n.eq_ignore_ascii_case("mtype") && v.eq_ignore_ascii_case("seg"))
    }

    fn validate_intact_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        if tag.starts_with("str:") {
            return true;
        }
        if !tag.eq_ignore_ascii_case("group")
            && !tag.eq_ignore_ascii_case("trans-unit")
            && !tag.eq_ignore_ascii_case("bin-unit")
        {
            return false;
        }
        atts.iter()
            .any(|(n, v)| n.eq_ignore_ascii_case("translate") && v.eq_ignore_ascii_case("no"))
    }

    fn validate_content_based_tag(&self, tag: &str, atts: &[(String, String)]) -> bool {
        tag == "mrk"
            && atts
                .iter()
                .any(|(n, v)| n == "mtype" && v == "protected")
    }

    fn handle_xml_tag(&self, tag: &mut XmlTag, translated: bool) {
        if tag.tag != "target" {
            return;
        }
        if let Some(attr) = tag.attrs.iter_mut().find(|a| a.name == "state") {
            let next = if self.change_state_to_needs_review {
                "needs-review-translation"
            } else {
                "translated"
            };
            if translated
                && (attr.value == "needs-translation" || attr.value == "needs-review-translation")
            {
                attr.value = next.to_string();
            } else if attr.value == "new" {
                attr.value = if translated {
                    next.to_string()
                } else {
                    "needs-translation".into()
                };
            }
        }
    }
}
