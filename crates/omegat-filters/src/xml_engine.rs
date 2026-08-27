//! Event-stream XML engine ported from Java `Handler` + `Entry` + `XMLWriter`.
//! Uses `quick-xml` as the SAX-like reader. Reconstruction is event-based,
//! not a tree walk or file-wide `find`.

use crate::xml_dialect::XmlDialect;
use crate::xml_entities::{prepare_xml, reconstruct_doctype_from_source, reject_self_nested_leaf_tags};
use crate::ProtectedPart;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::{HashMap, VecDeque};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagType {
    Begin,
    End,
    Alone,
}

#[derive(Clone, Debug)]
pub struct Attr {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct XmlTag {
    pub tag: String,
    pub shortcut: String,
    pub typ: TagType,
    pub attrs: Vec<Attr>,
    pub start_attrs: Vec<Attr>,
    pub index: i32,
}

impl XmlTag {
    fn new(tag: &str, shortcut: Option<&str>, typ: TagType, attrs: Vec<Attr>) -> Self {
        let shortcut = shortcut
            .map(|s| s.to_string())
            .unwrap_or_else(|| first_shortcut_char(tag));
        Self {
            tag: tag.to_string(),
            shortcut,
            typ,
            attrs,
            start_attrs: Vec::new(),
            index: 0,
        }
    }

    pub fn to_shortcut(&self) -> String {
        let mut buf = String::from("<");
        if self.typ == TagType::End {
            buf.push('/');
        }
        buf.push_str(&self.shortcut);
        buf.push_str(&self.index.to_string());
        if self.typ == TagType::Alone {
            buf.push('/');
        }
        buf.push('>');
        buf
    }

    pub fn to_original(&self) -> String {
        let mut buf = String::from("<");
        if self.typ == TagType::End {
            buf.push('/');
        }
        buf.push_str(&self.tag);
        for a in &self.attrs {
            buf.push(' ');
            buf.push_str(&a.name);
            buf.push_str("=\"");
            buf.push_str(&a.value);
            buf.push('"');
        }
        if self.typ == TagType::Alone {
            buf.push('/');
        }
        buf.push('>');
        buf
    }
}

fn java_trim_is_empty(text: &str) -> bool {
    text.chars().all(|c| (c as u32) <= 0x20)
}

fn first_shortcut_char(tag: &str) -> String {
    tag.chars().next().unwrap_or('f').to_string()
}

#[derive(Clone, Debug)]
pub enum Element {
    Text { text: String, in_cdata: bool },
    Tag(XmlTag),
    Intact {
        tag: XmlTag,
        inner: Vec<Element>,
        content_based: bool,
    },
    OutOfTurn { tag: XmlTag, inner: Vec<Element> },
    Comment(String),
    Pi { target: String, data: String },
    Doctype(String),
    Entity { name: String, value: String },
}

impl Element {
    fn is_meaningful_text(&self) -> bool {
        match self {
            // Java `String.trim()` only strips code points <= U+0020, not U+3000.
            Element::Text { text, .. } => !java_trim_is_empty(text),
            Element::Entity { value, .. } => !java_trim_is_empty(value),
            _ => false,
        }
    }

    fn as_tag(&self) -> Option<&XmlTag> {
        match self {
            Element::Tag(t) => Some(t),
            Element::Intact { tag, .. } | Element::OutOfTurn { tag, .. } => Some(tag),
            _ => None,
        }
    }

    fn as_tag_mut(&mut self) -> Option<&mut XmlTag> {
        match self {
            Element::Tag(t) => Some(t),
            Element::Intact { tag, .. } | Element::OutOfTurn { tag, .. } => Some(tag),
            _ => None,
        }
    }

    pub fn to_shortcut(&self) -> String {
        match self {
            Element::Text { text, .. } => text.clone(),
            Element::Entity { value, .. } => value.clone(),
            Element::Tag(t) => t.to_shortcut(),
            Element::Intact { tag, .. } | Element::OutOfTurn { tag, .. } => tag.to_shortcut(),
            Element::Comment(_) | Element::Pi { .. } | Element::Doctype(_) => {
                XmlTag::new("!", Some("cp"), TagType::Alone, Vec::new()).to_shortcut()
            }
        }
    }

    pub fn to_original(&self) -> String {
        match self {
            Element::Text { text, in_cdata } => {
                if *in_cdata {
                    format!("<![CDATA[{text}]]>")
                } else {
                    make_valid_xml(text)
                }
            }
            Element::Tag(t) => t.to_original(),
            Element::Intact { tag, inner, .. } => {
                let atts = attrs_string(&tag.attrs);
                format!(
                    "<{}{}>{}</{}>",
                    tag.tag,
                    atts,
                    inner.iter().map(|e| e.to_original()).collect::<String>(),
                    tag.tag
                )
            }
            Element::OutOfTurn { tag, inner } => {
                let atts = attrs_string(&tag.attrs);
                format!(
                    "<{}{}>{}</{}>",
                    tag.tag,
                    atts,
                    inner.iter().map(|e| e.to_original()).collect::<String>(),
                    tag.tag
                )
            }
            Element::Entity { name, .. } => format!("&{name};"),
            Element::Comment(c) => format!("<!--{c}-->"),
            Element::Pi { target, data } => {
                if data.is_empty() {
                    format!("<?{target}?>")
                } else {
                    format!("<?{target} {data}?>")
                }
            }
            Element::Doctype(d) => d.clone(),
        }
    }
}

fn attrs_string(attrs: &[Attr]) -> String {
    let mut buf = String::new();
    for a in attrs {
        buf.push(' ');
        buf.push_str(&a.name);
        buf.push_str("=\"");
        buf.push_str(&a.value);
        buf.push('"');
    }
    buf
}

pub fn default_construct_shortcuts(
    elements: &[Element],
    protected: &mut Vec<ProtectedPart>,
) -> String {
    protected.clear();
    let mut r = String::new();
    for el in elements {
        let shortcut = el.to_shortcut();
        r.push_str(&shortcut);
        if !matches!(el, Element::Text { .. } | Element::Entity { .. }) {
            protected.push(ProtectedPart {
                text: shortcut,
                details: el.to_original(),
            });
        }
    }
    r
}

pub fn make_valid_xml(plaintext: &str) -> String {
    let mut out = String::new();
    for ch in plaintext.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '>' => out.push_str("&gt;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            c if is_xml_invalid(c) => {}
            c => out.push(c),
        }
    }
    out
}

fn is_xml_invalid(c: char) -> bool {
    let cp = c as u32;
    !(cp == 0x9
        || cp == 0xA
        || cp == 0xD
        || (0x20..=0xD7FF).contains(&cp)
        || (0xE000..=0xFFFD).contains(&cp)
        || (0x10000..=0x10FFFF).contains(&cp))
}

/// Java `StringUtil.compressSpaces`.
pub fn compress_spaces(str: &str) -> String {
    let mut res = String::new();
    let mut wasspace = true;
    for ch in str.chars() {
        if ch.is_whitespace() {
            wasspace = true;
        } else {
            if wasspace && !res.is_empty() {
                res.push(' ');
            }
            res.push(ch);
            wasspace = false;
        }
    }
    res
}

#[derive(Clone, Copy)]
pub struct EngineConfig {
    pub remove_tags: bool,
    pub remove_spaces_nonseg: bool,
    pub preserve_spaces: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            remove_tags: true,
            remove_spaces_nonseg: true,
            preserve_spaces: false,
        }
    }
}

pub trait FilterHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]);
    fn tag_end(&mut self, path: &str);
    fn comment(&mut self, comment: &str);
    fn text(&mut self, text: &str);
    fn is_in_ignored(&self) -> bool;
    fn translate(&mut self, entry: &str, protected: &[ProtectedPart]) -> String;
}

#[derive(Default)]
struct Entry {
    elements: Vec<Element>,
    tags_detected: bool,
    first_good: i32,
    last_good: i32,
    translated: Option<Vec<Element>>,
}

impl Entry {
    fn clear(&mut self) {
        self.elements.clear();
        self.tags_detected = false;
        self.translated = None;
        self.first_good = 0;
        self.last_good = 0;
    }

    fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    fn add(&mut self, el: Element) {
        self.elements.push(el);
        self.tags_detected = false;
    }

    fn last_mut(&mut self) -> Option<&mut Element> {
        self.elements.last_mut()
    }

    fn detect_and_enumerate(&mut self, cfg: EngineConfig, dialect: &dyn XmlDialect) {
        if self.tags_detected {
            return;
        }
        if dialect.base().tags_aggregation_enabled {
            self.aggregate_tags();
        }
        self.detect_tags(cfg, dialect);
        self.tags_detected = true;
        let first = self.first_good;
        let last = self.last_good;
        self.enumerate_tags(first, last);
    }

    fn reset_tag_detected(&mut self) {
        self.tags_detected = false;
    }

    fn aggregate_tags(&mut self) {
        let mut new_els = Vec::new();
        let mut pending: Vec<Element> = Vec::new();
        for el in self.elements.drain(..) {
            if matches!(el, Element::Tag(_)) {
                pending.push(el);
            } else {
                new_els.append(&mut pending);
                new_els.push(el);
            }
        }
        new_els.append(&mut pending);
        self.elements = new_els;
    }

    fn detect_tags(&mut self, cfg: EngineConfig, dialect: &dyn XmlDialect) {
        let mut text_start = -1i32;
        for (i, el) in self.elements.iter().enumerate() {
            if el.is_meaningful_text() {
                text_start = i as i32;
                break;
            }
            if matches!(
                el,
                Element::Intact {
                    content_based: true,
                    ..
                }
            ) {
                text_start = i as i32;
            }
        }
        if text_start < 0 {
            self.first_good = -1;
            self.last_good = -2;
            return;
        }
        let mut text_end = text_start;
        for i in (0..self.elements.len()).rev() {
            if self.elements[i].is_meaningful_text() {
                text_end = i as i32;
                break;
            }
        }
        expand_content_based_pairs(&self.elements, &mut text_start, &mut text_end);

        let mut first_good = 0i32;
        let mut found = false;
        while first_good < text_start {
            let Some(good) = self.elements[first_good as usize].as_tag() else {
                first_good += 1;
                continue;
            };
            if good.typ != TagType::Begin {
                first_good += 1;
                continue;
            }
            let good_tag = good.tag.clone();
            let mut recursion = 1;
            for i in (first_good + 1)..text_end {
                if let Some(cand) = self.elements[i as usize].as_tag() {
                    if cand.tag == good_tag {
                        if cand.typ == TagType::Begin {
                            recursion += 1;
                        } else if cand.typ == TagType::End {
                            recursion -= 1;
                            if recursion == 0 {
                                if i > text_start {
                                    found = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            if found {
                break;
            }
            first_good += 1;
        }
        if !found {
            first_good = text_start;
        }

        let mut last_good = (self.elements.len() as i32) - 1;
        found = false;
        while last_good > text_end {
            let Some(good) = self.elements[last_good as usize].as_tag() else {
                last_good -= 1;
                continue;
            };
            if good.typ != TagType::End {
                last_good -= 1;
                continue;
            }
            let good_tag = good.tag.clone();
            let mut recursion = 1;
            for i in ((text_start + 1)..last_good).rev() {
                if let Some(cand) = self.elements[i as usize].as_tag() {
                    if cand.tag == good_tag {
                        if cand.typ == TagType::End {
                            recursion += 1;
                        } else if cand.typ == TagType::Begin {
                            recursion -= 1;
                            if recursion == 0 {
                                if i < text_end {
                                    found = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            if found {
                break;
            }
            last_good -= 1;
        }
        if !found {
            last_good = text_end;
        }

        if !cfg.remove_tags {
            let mut i = first_good - 1;
            while i >= 0 {
                if let Some(tag) = self.elements[i as usize].as_tag() {
                    if is_paragraph_name(dialect, &tag.tag) {
                        break;
                    }
                    first_good = i;
                }
                i -= 1;
            }
            let mut i = last_good + 1;
            while i < self.elements.len() as i32 {
                if let Some(tag) = self.elements[i as usize].as_tag() {
                    if is_paragraph_name(dialect, &tag.tag) {
                        break;
                    }
                    last_good = i;
                }
                i += 1;
            }
        }
        if !cfg.remove_spaces_nonseg {
            let mut i = first_good - 1;
            while i >= 0 {
                let el = &self.elements[i as usize];
                if let Some(tag) = el.as_tag() {
                    if is_paragraph_name(dialect, &tag.tag) {
                        break;
                    }
                }
                if matches!(el, Element::Text { .. }) && !el.is_meaningful_text() {
                    first_good = i;
                }
                i -= 1;
            }
            let mut i = last_good + 1;
            while i < self.elements.len() as i32 {
                let el = &self.elements[i as usize];
                if let Some(tag) = el.as_tag() {
                    if is_paragraph_name(dialect, &tag.tag) {
                        break;
                    }
                }
                if matches!(el, Element::Text { .. }) && !el.is_meaningful_text() {
                    last_good = i;
                }
                i += 1;
            }
        }
        self.first_good = first_good;
        self.last_good = last_good;
    }

    fn enumerate_tags(&mut self, first_good: i32, last_good: i32) {
        if first_good < 0 || last_good < first_good {
            return;
        }
        let mut n = 0i32;
        for i in first_good..=last_good {
            let i = i as usize;
            let tag_name;
            let typ;
            {
                let Some(tag) = self.elements[i].as_tag() else {
                    continue;
                };
                tag_name = tag.tag.clone();
                typ = tag.typ;
            }
            if typ == TagType::Alone || typ == TagType::Begin {
                if let Some(tag) = self.elements[i].as_tag_mut() {
                    tag.index = n;
                }
                n += 1;
            } else if typ == TagType::End {
                if let Some(tag) = self.elements[i].as_tag_mut() {
                    tag.index = -1;
                }
                let mut recursion = 1;
                let mut found_idx = None;
                for j in (first_good as usize..i).rev() {
                    if let Some(other) = self.elements[j].as_tag() {
                        if other.tag == tag_name {
                            if other.typ == TagType::End {
                                recursion += 1;
                            } else if other.typ == TagType::Begin {
                                recursion -= 1;
                                if recursion == 0 {
                                    found_idx = Some(other.index);
                                    break;
                                }
                            }
                        }
                    }
                }
                if let Some(tag) = self.elements[i].as_tag_mut() {
                    if let Some(idx) = found_idx {
                        tag.index = idx;
                    } else {
                        tag.index = n;
                        n += 1;
                    }
                }
            }
        }
    }

    fn source_to_shortcut(
        &mut self,
        cfg: EngineConfig,
        dialect: &dyn XmlDialect,
        protected: &mut Vec<ProtectedPart>,
    ) -> String {
        self.detect_and_enumerate(cfg, dialect);
        if self.first_good <= self.last_good && self.first_good >= 0 {
            let start = self.first_good as usize;
            let end = self.last_good as usize + 1;
            dialect.construct_shortcuts(&self.elements[start..end], protected)
        } else {
            String::new()
        }
    }

    fn source_to_original(&self) -> String {
        self.elements.iter().map(|e| e.to_original()).collect()
    }

    fn set_translation(
        &mut self,
        translation: &str,
        cfg: EngineConfig,
        dialect: &dyn XmlDialect,
        protected: &[ProtectedPart],
    ) {
        self.detect_and_enumerate(cfg, dialect);
        let src = if self.first_good <= self.last_good && self.first_good >= 0 {
            let start = self.first_good as usize;
            let end = self.last_good as usize + 1;
            let mut tmp = Vec::new();
            dialect.construct_shortcuts(&self.elements[start..end], &mut tmp)
        } else {
            String::new()
        };
        if src == translation || self.first_good < 0 {
            self.translated = None;
            return;
        }
        self.translated = Some(recover_tags(
            translation,
            &self.elements,
            self.first_good,
            self.last_good,
            protected,
        ));
    }

    fn translation_to_original(&self) -> String {
        if self.first_good < 0 {
            return self.source_to_original();
        }
        if let Some(ref tr) = self.translated {
            let mut buf = String::new();
            for i in 0..self.first_good as usize {
                buf.push_str(&self.elements[i].to_original());
            }
            for e in tr {
                buf.push_str(&e.to_original());
            }
            for i in (self.last_good as usize + 1)..self.elements.len() {
                buf.push_str(&self.elements[i].to_original());
            }
            buf
        } else {
            self.source_to_original()
        }
    }

    fn into_translation_elements(self) -> Vec<Element> {
        if self.first_good < 0 {
            return self.elements;
        }
        let Some(translated) = self.translated else {
            return self.elements;
        };
        let mut elements = Vec::with_capacity(
            self.first_good as usize
                + translated.len()
                + self
                    .elements
                    .len()
                    .saturating_sub(self.last_good as usize + 1),
        );
        elements.extend(self.elements[..self.first_good as usize].iter().cloned());
        elements.extend(translated);
        elements.extend(
            self.elements[(self.last_good as usize + 1)..]
                .iter()
                .cloned(),
        );
        elements
    }
}

fn recover_tags(
    translation: &str,
    source: &[Element],
    first_good: i32,
    last_good: i32,
    _protected: &[ProtectedPart],
) -> Vec<Element> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let bytes = translation.as_bytes();
    while pos < translation.len() {
        if let Some(rel) = translation[pos..].find('<') {
            if rel > 0 {
                out.push(Element::Text {
                    text: translation[pos..pos + rel].to_string(),
                    in_cdata: false,
                });
            }
            let start = pos + rel;
            if let Some(end_rel) = translation[start..].find('>') {
                let tag_s = &translation[start..start + end_rel + 1];
                let mut found = false;
                if first_good >= 0 && last_good >= first_good {
                    for j in first_good as usize..=last_good as usize {
                        if let Some(long) = source[j].as_tag() {
                            if long.to_shortcut() == tag_s {
                                out.push(source[j].clone());
                                found = true;
                                break;
                            }
                        }
                    }
                }
                if !found {
                    out.push(Element::Text {
                        text: tag_s.to_string(),
                        in_cdata: false,
                    });
                }
                pos = start + end_rel + 1;
            } else {
                out.push(Element::Text {
                    text: translation[start..].to_string(),
                    in_cdata: false,
                });
                break;
            }
        } else {
            if pos < translation.len() {
                out.push(Element::Text {
                    text: translation[pos..].to_string(),
                    in_cdata: false,
                });
            }
            break;
        }
        let _ = bytes;
    }
    out
}

fn content_based_pair_id(el: &Element) -> Option<String> {
    let Element::Intact {
        tag,
        content_based: true,
        ..
    } = el
    else {
        return None;
    };
    if tag.tag != "bpt" && tag.tag != "ept" {
        return None;
    }
    tag.attrs
        .iter()
        .find(|a| a.name == "rid" || a.name == "id" || a.name == "i")
        .map(|a| a.value.clone())
}

fn expand_content_based_pairs(elements: &[Element], text_start: &mut i32, text_end: &mut i32) {
    let start = *text_start;
    let end = *text_end;
    for i in start..=end {
        let Some(id) = content_based_pair_id(&elements[i as usize]) else {
            continue;
        };
        for j in (0..start).rev() {
            if content_based_pair_id(&elements[j as usize]).as_deref() == Some(id.as_str()) {
                *text_start = j;
            }
        }
        for j in (end + 1)..elements.len() as i32 {
            if content_based_pair_id(&elements[j as usize]).as_deref() == Some(id.as_str()) {
                *text_end = j;
            }
        }
    }
}

fn is_paragraph_name(dialect: &dyn XmlDialect, tag: &str) -> bool {
    dialect.base().paragraph_tags.contains(tag) || dialect.base().preformat_tags.contains(tag)
}

fn attr_value<'a>(atts: &'a [(String, String)], name: &str) -> Option<&'a str> {
    atts.iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

struct Handler<'a> {
    dialect: &'a dyn XmlDialect,
    hooks: &'a mut dyn FilterHooks,
    cfg: EngineConfig,
    output: String,
    entry: Entry,
    outofturn: Vec<Entry>,
    intact: Option<Entry>,
    intact_name: Option<String>,
    intact_attrs: Vec<(String, String)>,
    xml_tag_name: VecDeque<String>,
    xml_tag_attrs: VecDeque<Vec<Attr>>,
    paragraph_tag_name: VecDeque<String>,
    paragraph_tag_attrs: VecDeque<Vec<(String, String)>>,
    preformat_tag_name: VecDeque<String>,
    preformat_tag_attrs: VecDeque<Vec<(String, String)>>,
    translatable_tag_name: VecDeque<String>,
    current_path: VecDeque<String>,
    space_preserve: bool,
    in_cdata: bool,
    seen_root: bool,
    entities: HashMap<String, String>,
}

impl<'a> Handler<'a> {
    fn new(dialect: &'a dyn XmlDialect, hooks: &'a mut dyn FilterHooks, cfg: EngineConfig) -> Self {
        Self {
            dialect,
            hooks,
            cfg,
            output: String::new(),
            entry: Entry::default(),
            outofturn: Vec::new(),
            intact: None,
            intact_name: None,
            intact_attrs: Vec::new(),
            xml_tag_name: VecDeque::new(),
            xml_tag_attrs: VecDeque::new(),
            paragraph_tag_name: VecDeque::new(),
            paragraph_tag_attrs: VecDeque::new(),
            preformat_tag_name: VecDeque::new(),
            preformat_tag_attrs: VecDeque::new(),
            translatable_tag_name: VecDeque::new(),
            current_path: VecDeque::new(),
            space_preserve: false,
            in_cdata: false,
            seen_root: false,
            entities: HashMap::new(),
        }
    }

    fn collecting_intact(&self) -> bool {
        self.intact.is_some()
    }

    fn collecting_oot(&self) -> bool {
        !self.outofturn.is_empty()
    }

    fn curr_entry(&mut self) -> &mut Entry {
        if self.intact.is_some() {
            self.intact.as_mut().unwrap()
        } else if let Some(e) = self.outofturn.last_mut() {
            e
        } else {
            &mut self.entry
        }
    }

    fn is_translatable_tag(&self) -> bool {
        !self.translatable_tag_name.is_empty()
    }

    fn is_space_preserving(&self) -> bool {
        self.cfg.preserve_spaces || self.space_preserve || self.dialect.base().force_space_preserving
    }

    fn construct_path(&self) -> String {
        let mut path = String::new();
        for t in &self.current_path {
            path.push('/');
            path.push_str(t);
        }
        path
    }

    fn get_shortcut(&self, tag: &str) -> Option<&str> {
        self.dialect.base().shortcuts.get(tag).map(|s| s.as_str())
    }

    fn set_translatable_tag(&mut self, tag: &str, atts: &[(String, String)]) {
        if !self.is_translatable_tag() {
            if self.dialect.validate_translatable_tag(tag, atts) {
                self.translatable_tag_name.push_back(tag.to_string());
            }
        } else {
            self.translatable_tag_name.push_back(tag.to_string());
        }
    }

    fn remove_translatable_tag(&mut self) {
        self.translatable_tag_name.pop_back();
    }

    fn is_paragraph_tag_start(&mut self, tag: &str, atts: &[(String, String)]) -> bool {
        self.paragraph_tag_name.push_back(tag.to_string());
        self.paragraph_tag_attrs.push_back(atts.to_vec());
        self.preformat_tag_name.push_back(tag.to_string());
        self.preformat_tag_attrs.push_back(atts.to_vec());
        if self.dialect.base().paragraph_tags.contains(tag) || self.is_preformat(tag, Some(atts)) {
            true
        } else {
            self.dialect.validate_paragraph_tag(tag, atts)
        }
    }

    fn is_paragraph_tag_end(&mut self, tag: &str) -> bool {
        if self.dialect.base().paragraph_tags.contains(tag) || self.is_preformat_end(tag) {
            return true;
        }
        let atts = if self.paragraph_tag_name.back().map(|s| s.as_str()) == Some(tag) {
            self.paragraph_tag_name.pop_back();
            self.paragraph_tag_attrs.pop_back().unwrap_or_default()
        } else {
            Vec::new()
        };
        self.dialect.validate_paragraph_tag(tag, &atts)
    }

    fn is_preformat(&self, tag: &str, atts: Option<&[(String, String)]>) -> bool {
        if self.dialect.base().preformat_tags.contains(tag) {
            return true;
        }
        self.dialect
            .validate_preformat_tag(tag, atts.unwrap_or(&[]))
    }

    fn is_preformat_end(&mut self, tag: &str) -> bool {
        if self.dialect.base().preformat_tags.contains(tag) {
            return true;
        }
        let atts = if self.preformat_tag_name.back().map(|s| s.as_str()) == Some(tag) {
            self.preformat_tag_name.pop_back();
            self.preformat_tag_attrs.pop_back().unwrap_or_default()
        } else {
            Vec::new()
        };
        self.dialect.validate_preformat_tag(tag, &atts)
    }

    fn is_intact(&self, tag: &str, atts: Option<&[(String, String)]>) -> bool {
        if self.dialect.base().intact_tags.contains(tag) {
            return true;
        }
        let atts = atts.unwrap_or(self.intact_attrs.as_slice());
        self.dialect.validate_intact_tag(tag, atts)
    }

    fn is_content_based(&self, tag: &str, atts: Option<&[(String, String)]>) -> bool {
        if self.dialect.base().content_based_tags.contains_key(tag) {
            return true;
        }
        if atts.is_none() && self.intact_name.as_deref() == Some(tag) {
            return self.dialect.validate_content_based_tag(tag, &self.intact_attrs);
        }
        atts.map(|a| self.dialect.validate_content_based_tag(tag, a))
            .unwrap_or(false)
    }

    fn is_out_of_turn(&self, tag: &str) -> bool {
        self.dialect.base().out_of_turn_tags.contains(tag)
    }

    fn is_translatable_attribute(&self, tag: &str, name: &str) -> bool {
        self.dialect.base().translatable_attributes.contains(name)
            || self
                .dialect
                .base()
                .translatable_tag_attributes
                .get(tag)
                .map(|s| s.contains(name))
                .unwrap_or(false)
    }

    fn set_space_preserving(&mut self, atts: &[(String, String)]) {
        if self.dialect.base().force_space_preserving {
            self.space_preserve = true;
            return;
        }
        if attr_value(atts, "xml:space")
            .map(|v| v.eq_ignore_ascii_case("preserve"))
            .unwrap_or(false)
        {
            self.space_preserve = true;
        }
    }

    fn queue_entity_or_text(&mut self, s: &str) {
        if let Some(name) = s
            .strip_prefix('&')
            .and_then(|t| t.strip_suffix(';'))
            .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':'))
        {
            if self.entities.contains_key(name) {
                self.queue_general_ref(name);
                return;
            }
        }
        self.queue_text(s);
    }

    fn queue_general_ref(&mut self, name: &str) {
        if let Some(value) = self.entities.get(name).cloned() {
            if !self.hooks.is_in_ignored() {
                self.hooks.text(&value);
            }
            self.curr_entry().add(Element::Entity {
                name: name.to_string(),
                value,
            });
        } else {
            self.queue_text(&format!("&{name};"));
        }
    }

    fn queue_text(&mut self, s: &str) {
        let s = normalize_eol(s);
        if !self.hooks.is_in_ignored() {
            self.hooks.text(&s);
        }
        let in_cdata = self.in_cdata;
        let entry = self.curr_entry();
        if let Some(Element::Text {
            text,
            in_cdata: was,
        }) = entry.last_mut()
        {
            if *was == in_cdata {
                text.push_str(&s);
                entry.reset_tag_detected();
                return;
            }
        }
        entry.add(Element::Text {
            text: s,
            in_cdata,
        });
    }

    fn queue_comment(&mut self, comment: &str) {
        let comment = normalize_eol(comment);
        if !self.hooks.is_in_ignored() {
            self.hooks.comment(&comment);
        }
        self.curr_entry().add(Element::Comment(comment));
    }

    fn queue_pi(&mut self, target: &str, data: &str) {
        self.curr_entry().add(Element::Pi {
            target: target.to_string(),
            data: data.to_string(),
        });
    }

    fn queue_tag(&mut self, tag: &str, attrs: &[(String, String)]) {
        self.set_translatable_tag(tag, attrs);
        self.set_space_preserving(attrs);
        if !self.collecting_intact() {
            let content_based = self.is_content_based(tag, Some(attrs));
            if content_based || self.is_intact(tag, Some(attrs)) {
                let shortcut = self.get_shortcut(tag).map(|s| s.to_string());
                let t = XmlTag::new(tag, shortcut.as_deref(), TagType::Alone, convert_attrs(attrs));
                self.intact_name = Some(tag.to_string());
                self.intact_attrs = attrs.to_vec();
                self.curr_entry().add(Element::Intact {
                    tag: t,
                    inner: Vec::new(),
                    content_based,
                });
                self.intact = Some(Entry::default());
                return;
            }
        }
        self.xml_tag_name.push_back(tag.to_string());
        self.xml_tag_attrs.push_back(convert_attrs(attrs));
        let shortcut = self.get_shortcut(tag).map(|s| s.to_string());
        let mut t = XmlTag::new(tag, shortcut.as_deref(), TagType::Begin, convert_attrs(attrs));
        if !self.collecting_intact() {
            self.process_translatable_attributes(&mut t, tag);
        }
        self.curr_entry().add(Element::Tag(t));
    }

    fn process_translatable_attributes(&mut self, xmltag: &mut XmlTag, tag: &str) {
        let pairs: Vec<(String, String)> = xmltag
            .attrs
            .iter()
            .map(|a| (a.name.clone(), unescape_xml(&a.value)))
            .collect();
        for attr in &mut xmltag.attrs {
            if self.is_translatable_attribute(tag, &attr.name)
                && self
                    .dialect
                    .validate_translatable_tag_attribute(tag, &attr.name, &pairs)
            {
                let unescaped = unescape_xml(&attr.value);
                let translated = self.hooks.translate(&unescaped, &[]);
                attr.value = make_valid_xml(&translated);
            }
        }
    }

    fn queue_end_tag(&mut self, tag: &str) {
        let closing_required = self.dialect.base().closing_tag_required;
        let entry = self.curr_entry();
        let collapse = if let Some(Element::Tag(t)) = entry.elements.last() {
            t.tag == tag && t.typ == TagType::Begin && !closing_required
        } else {
            false
        };
        if collapse {
            if self.xml_tag_name.back().map(|s| s.as_str()) == Some(tag) {
                self.xml_tag_name.pop_back();
                self.xml_tag_attrs.pop_back();
            }
            if let Some(Element::Tag(t)) = self.curr_entry().elements.last_mut() {
                t.typ = TagType::Alone;
            }
        } else {
            let shortcut = self.get_shortcut(tag).map(|s| s.to_string());
            let mut t = XmlTag::new(tag, shortcut.as_deref(), TagType::End, Vec::new());
            if self.xml_tag_name.back().map(|s| s.as_str()) == Some(tag) {
                self.xml_tag_name.pop_back();
                t.start_attrs = self.xml_tag_attrs.pop_back().unwrap_or_default();
            }
            self.curr_entry().add(Element::Tag(t));
        }
    }

    fn translate_but_dont_flush(&mut self) {
        if self.curr_entry().is_empty() {
            return;
        }
        let mut protected = Vec::new();
        let src = {
            let cfg = self.cfg;
            let dialect = self.dialect;
            self.curr_entry()
                .source_to_shortcut(cfg, dialect, &mut protected)
        };
        let lead_is_tag = self
            .curr_entry()
            .elements
            .first()
            .and_then(|e| e.as_tag())
            .map(|t| t.tag.clone());
        let lead_is_pre = lead_is_tag
            .as_deref()
            .map(|t| self.is_preformat(t, None))
            .unwrap_or(false);
        let mut is_translated = true;
        let translation = if (lead_is_pre || self.is_space_preserving())
            && self.is_translatable_tag()
            && !src.is_empty()
        {
            self.space_preserve = false;
            self.hooks.translate(&src, &protected)
        } else {
            let compressed = if self.cfg.remove_spaces_nonseg {
                compress_spaces(&src)
            } else {
                src.clone()
            };
            let mut translation = if self.is_translatable_tag() {
                self.hooks.translate(&compressed, &protected)
            } else {
                compressed.clone()
            };
            if compressed == translation {
                translation = src;
                is_translated = false;
            }
            translation
        };
        let dialect = self.dialect;
        if let Some(Element::Tag(t)) = self.curr_entry().elements.first_mut() {
            dialect.handle_xml_tag(t, is_translated);
        }
        let cfg = self.cfg;
        let dialect = self.dialect;
        self.curr_entry()
            .set_translation(&translation, cfg, dialect, &protected);
    }

    fn translate_and_flush(&mut self) {
        self.translate_but_dont_flush();
        let written = self.curr_entry().translation_to_original();
        self.output.push_str(&written);
        self.curr_entry().clear();
    }

    fn start(&mut self, tag: &str, attrs: &[(String, String)]) {
        let prev_ignored = self.hooks.is_in_ignored();
        self.current_path.push_back(tag.to_string());
        let path = self.construct_path();
        self.hooks.tag_start(&path, attrs);

        if !self.hooks.is_in_ignored() {
            if self.is_out_of_turn(tag) {
                let shortcut = self.get_shortcut(tag).map(|s| s.to_string());
                let t = XmlTag::new(tag, shortcut.as_deref(), TagType::Alone, convert_attrs(attrs));
                self.curr_entry().add(Element::OutOfTurn {
                    tag: t,
                    inner: Vec::new(),
                });
                self.outofturn.push(Entry::default());
            } else {
                if self.is_paragraph_tag_start(tag, attrs)
                    && !self.collecting_oot()
                    && !self.collecting_intact()
                {
                    self.translate_and_flush();
                }
                self.queue_tag(tag, attrs);
            }
        } else {
            if !prev_ignored {
                self.translate_and_flush();
            }
            self.set_space_preserving(attrs);
            self.xml_tag_name.push_back(tag.to_string());
            self.xml_tag_attrs.push_back(convert_attrs(attrs));
            let shortcut = self.get_shortcut(tag).map(|s| s.to_string());
            let t = XmlTag::new(tag, shortcut.as_deref(), TagType::Begin, convert_attrs(attrs));
            self.curr_entry().add(Element::Tag(t));
        }
        self.seen_root = true;
    }

    fn end(&mut self, tag: &str) {
        let prev_ignored = self.hooks.is_in_ignored();
        if !self.hooks.is_in_ignored() {
            if self.collecting_intact()
                && self.intact_name.as_deref() == Some(tag)
                && (self.is_intact(tag, None) || self.is_content_based(tag, None))
            {
                let inner = self.intact.take().map(|e| e.elements).unwrap_or_default();
                self.intact_name = None;
                self.intact_attrs.clear();
                if let Some(Element::Intact { inner: dest, .. }) = self.curr_entry().elements.last_mut()
                {
                    *dest = inner;
                }
                self.remove_translatable_tag();
            } else if self.collecting_oot() && self.is_out_of_turn(tag) {
                self.translate_but_dont_flush();
                let finished = self.outofturn.pop();
                if let Some(fin) = finished {
                    if let Some(Element::OutOfTurn { inner, .. }) =
                        self.curr_entry().elements.last_mut()
                    {
                        *inner = fin.into_translation_elements();
                    }
                }
            } else {
                self.queue_end_tag(tag);
                if self.is_paragraph_tag_end(tag)
                    && !self.collecting_oot()
                    && !self.collecting_intact()
                {
                    self.translate_and_flush();
                }
                self.remove_translatable_tag();
            }
        } else {
            self.queue_end_tag(tag);
        }

        let path = self.construct_path();
        self.hooks.tag_end(&path);
        while let Some(popped) = self.current_path.pop_back() {
            if popped == tag {
                break;
            }
        }
        if !self.hooks.is_in_ignored() && prev_ignored {
            let written = self.curr_entry().translation_to_original();
            self.output.push_str(&written);
            self.curr_entry().clear();
        }
    }
}

fn convert_attrs(attrs: &[(String, String)]) -> Vec<Attr> {
    attrs
        .iter()
        .map(|(n, v)| Attr {
            name: make_valid_xml(n),
            value: make_valid_xml(v),
        })
        .collect()
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub struct ProcessResult {
    pub output: String,
}

pub fn process_xml(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
) -> Result<ProcessResult, String> {
    process_xml_ex(raw, dialect, hooks, cfg, None, false)
}

pub fn process_xml_ex(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
    base: Option<&Path>,
    inline_system: bool,
) -> Result<ProcessResult, String> {
    let bom_utf8 = raw.starts_with('\u{feff}');
    let raw_no_bom = raw.trim_start_matches('\u{feff}');
    reject_self_nested_leaf_tags(raw_no_bom)?;
    let prep = prepare_xml(raw_no_bom, base, inline_system)?;
    let raw = prep.text.as_str();
    let encoding = if bom_utf8 {
        Some("UTF-8".to_string())
    } else {
        detect_encoding(raw)
    };
    let eol = detect_eol(raw);
    let header = if let Some(enc) = encoding {
        format!("<?xml version=\"1.0\" encoding=\"{enc}\"?>")
    } else {
        "<?xml version=\"1.0\"?>".to_string()
    };

    if raw.chars().any(is_xml_invalid) {
        return Err("invalid XML character".into());
    }
    let mut handler = Handler::new(dialect, hooks, cfg);
    handler.entities = prep.internal.clone();
    if handler.entities.is_empty() {
        handler.entities = parse_internal_entities(raw);
    }
    handler.output.push_str("<?xml version=\"1.0\"?>\n");

    let mut reader = Reader::from_str(raw);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = true;
    config.check_end_names = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let (name, attrs) = decode_start(&e, &reader);
                handler.start(&name, &attrs);
            }
            Ok(Event::End(e)) => {
                let name = qname_string(e.name().as_ref());
                handler.end(&name);
            }
            Ok(Event::Empty(e)) => {
                let (name, attrs) = decode_start(&e, &reader);
                handler.start(&name, &attrs);
                handler.end(&name);
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).into_owned());
                // Xerces does not report prolog whitespace or trailing misc text
                // after the root element into the translation buffer.
                if handler.seen_root && !handler.current_path.is_empty() {
                    handler.queue_entity_or_text(&text);
                }
            }
            Ok(Event::CData(t)) => {
                handler.in_cdata = true;
                let text = String::from_utf8_lossy(t.as_ref()).into_owned();
                handler.queue_text(&text);
                handler.in_cdata = false;
            }
            Ok(Event::Comment(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).into_owned();
                handler.queue_comment(&text);
            }
            Ok(Event::PI(t)) => {
                let raw_pi = String::from_utf8_lossy(t.as_ref()).into_owned();
                let (target, data) = split_pi(&raw_pi);
                handler.queue_pi(&target, &data);
            }
            Ok(Event::DocType(t)) => {
                let body = String::from_utf8_lossy(t.as_ref()).into_owned();
                let reconstructed = reconstruct_doctype_from_source(&body);
                handler.curr_entry().add(Element::Doctype(reconstructed));
            }
            Ok(Event::Decl(_)) => {}
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
        }
        buf.clear();
    }
    handler.translate_and_flush();
    handler.translate_and_flush();

    let mut out = handler.output;
    if let Some(start) = out.find("<?xml") {
        if let Some(end_rel) = out[start..].find("?>") {
            let end = start + end_rel + 2;
            out.replace_range(start..end, &header);
        }
    } else {
        out.insert_str(0, &format!("{header}\n"));
    }
    out = normalize_eol(&out);
    if eol != "\n" {
        out = out.replace('\n', &eol);
    }
    Ok(ProcessResult { output: out })
}

fn decode_start(e: &BytesStart<'_>, reader: &Reader<&[u8]>) -> (String, Vec<(String, String)>) {
    let name = qname_string(e.name().as_ref());
    let mut attrs = Vec::new();
    for a in e.attributes() {
        if let Ok(a) = a {
            let key = qname_string(a.key.as_ref());
            let val = a
                .decode_and_unescape_value(reader.decoder())
                .map(|c| c.into_owned())
                .unwrap_or_default();
            // XML 1.0 §3.3.3: normalize newlines/tabs in attribute values to space.
            let val = val.replace("\r\n", " ").replace(['\n', '\r', '\t'], " ");
            attrs.push((key, val));
        }
    }
    (name, attrs)
}

fn qname_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn split_pi(raw: &str) -> (String, String) {
    let raw = raw.trim();
    if let Some((t, d)) = raw.split_once(char::is_whitespace) {
        (t.to_string(), d.trim().to_string())
    } else {
        (raw.to_string(), String::new())
    }
}

fn detect_encoding(raw: &str) -> Option<String> {
    let re = regex::Regex::new(r#"<\?xml.*?encoding\s*=\s*"(\S+?)".*?\?>"#).ok()?;
    re.captures(raw)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn parse_internal_entities(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let re = regex::Regex::new(r#"<!ENTITY\s+(\w+)\s+"([^"]*)""#).unwrap();
    for cap in re.captures_iter(raw) {
        out.insert(cap[1].to_string(), cap[2].to_string());
    }
    out
}

fn detect_eol(raw: &str) -> String {
    if raw.contains("\r\n") {
        "\r\n".into()
    } else if raw.contains('\r') {
        "\r".into()
    } else {
        "\n".into()
    }
}
