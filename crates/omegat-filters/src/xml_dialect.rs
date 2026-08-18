//! Java `org.omegat.filters3.xml.XMLDialect` / `DefaultXMLDialect`.
//! Each filters3 dialect file owns its tag sets; this is the shared behaviour,
//! not a single table of all formats.

use crate::xml_engine::{default_construct_shortcuts, Element};
use crate::ProtectedPart;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstraintKind {
    Doctype = 1,
    PublicDoctype = 2,
    SystemDoctype = 3,
    Root = 4,
    Xmlns = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentTagType {
    Begin,
    End,
    Alone,
}

#[derive(Clone, Debug, Default)]
pub struct DefaultXmlDialect {
    pub paragraph_tags: HashSet<String>,
    pub preformat_tags: HashSet<String>,
    pub intact_tags: HashSet<String>,
    pub out_of_turn_tags: HashSet<String>,
    pub translatable_attributes: HashSet<String>,
    pub translatable_tag_attributes: HashMap<String, HashSet<String>>,
    pub content_based_tags: HashMap<String, ContentTagType>,
    pub shortcuts: HashMap<String, String>,
    pub constraints: HashMap<ConstraintKind, String>,
    pub closing_tag_required: bool,
    pub tags_aggregation_enabled: bool,
    pub force_space_preserving: bool,
}

impl DefaultXmlDialect {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define_paragraph_tags(&mut self, tags: &[&str]) {
        for t in tags {
            self.paragraph_tags.insert((*t).to_string());
        }
    }

    pub fn define_preformat_tags(&mut self, tags: &[&str]) {
        for t in tags {
            self.preformat_tags.insert((*t).to_string());
        }
    }

    pub fn define_intact_tags(&mut self, tags: &[&str]) {
        for t in tags {
            self.intact_tags.insert((*t).to_string());
        }
    }

    pub fn define_intact_tag(&mut self, tag: &str) {
        self.intact_tags.insert(tag.to_string());
    }

    pub fn define_out_of_turn_tags(&mut self, tags: &[&str]) {
        for t in tags {
            self.out_of_turn_tags.insert((*t).to_string());
        }
    }

    pub fn define_out_of_turn_tag(&mut self, tag: &str) {
        self.out_of_turn_tags.insert(tag.to_string());
    }

    pub fn define_translatable_attributes(&mut self, attrs: &[&str]) {
        for a in attrs {
            self.translatable_attributes.insert((*a).to_string());
        }
    }

    pub fn define_translatable_attribute(&mut self, attr: &str) {
        self.translatable_attributes.insert(attr.to_string());
    }

    pub fn define_translatable_tag_attribute(&mut self, tag: &str, attr: &str) {
        self.translatable_tag_attributes
            .entry(tag.to_string())
            .or_default()
            .insert(attr.to_string());
    }

    pub fn define_translatable_tag_attributes(&mut self, tag: &str, attrs: &[&str]) {
        for a in attrs {
            self.define_translatable_tag_attribute(tag, a);
        }
    }

    pub fn define_translatable_tags_attribute(&mut self, tags: &[&str], attr: &str) {
        for t in tags {
            self.define_translatable_tag_attribute(t, attr);
        }
    }

    pub fn define_content_based_tag(&mut self, tag: &str, typ: ContentTagType) {
        self.content_based_tags.insert(tag.to_string(), typ);
    }

    pub fn define_shortcut(&mut self, tag: &str, shortcut: &str) {
        self.shortcuts.insert(tag.to_string(), shortcut.to_string());
    }

    pub fn define_shortcuts(&mut self, mappings: &[&str]) {
        let mut i = 0;
        while i + 1 < mappings.len() {
            self.define_shortcut(mappings[i], mappings[i + 1]);
            i += 2;
        }
    }

    pub fn define_constraint(&mut self, kind: ConstraintKind, pattern: &str) {
        self.constraints.insert(kind, pattern.to_string());
    }
}

impl XmlDialect for DefaultXmlDialect {
    fn base(&self) -> &DefaultXmlDialect {
        self
    }
}

/// Java `XMLDialect` instance behaviour. Defaults match `DefaultXMLDialect`.
pub trait XmlDialect {
    fn base(&self) -> &DefaultXmlDialect;

    fn validate_intact_tag(&self, _tag: &str, _atts: &[(String, String)]) -> bool {
        false
    }

    fn validate_translatable_tag(&self, _tag: &str, _atts: &[(String, String)]) -> bool {
        true
    }

    fn validate_paragraph_tag(&self, _tag: &str, _atts: &[(String, String)]) -> bool {
        false
    }

    fn validate_preformat_tag(&self, _tag: &str, _atts: &[(String, String)]) -> bool {
        false
    }

    fn validate_content_based_tag(&self, _tag: &str, _atts: &[(String, String)]) -> bool {
        false
    }

    fn validate_translatable_tag_attribute(
        &self,
        _tag: &str,
        _attribute: &str,
        _atts: &[(String, String)],
    ) -> bool {
        true
    }

    fn handle_xml_tag(&self, _tag: &mut crate::xml_engine::XmlTag, _translated: bool) {}

    fn construct_shortcuts(
        &self,
        elements: &[Element],
        protected: &mut Vec<ProtectedPart>,
    ) -> String {
        default_construct_shortcuts(elements, protected)
    }
}

/// Java `XMLFilter.isFileSupported` constraint check on a read-ahead buffer.
pub fn is_xml_supported(raw: &str, dialect: &dyn XmlDialect) -> bool {
    let c = &dialect.base().constraints;
    if c.is_empty() {
        return true;
    }
    let doctype_re = regex::Regex::new(r#"<!DOCTYPE\s+(\w+)\s+(PUBLIC\s+"(-//.*)"\s+)?"#).unwrap();
    if let Some(m) = doctype_re.captures(raw) {
        if let Some(pat) = c.get(&ConstraintKind::Doctype) {
            let name = m.get(1).map(|g| g.as_str()).unwrap_or("");
            if !regex_full(pat, name) {
                return false;
            }
        }
        if let Some(pat) = c.get(&ConstraintKind::PublicDoctype) {
            let public = m.get(3).map(|g| g.as_str()).unwrap_or("");
            if public.is_empty() || !regex_full(pat, public) {
                return false;
            }
        }
        if let Some(pat) = c.get(&ConstraintKind::SystemDoctype) {
            let _ = pat;
        }
    } else if c.contains_key(&ConstraintKind::Doctype)
        || c.contains_key(&ConstraintKind::PublicDoctype)
        || c.contains_key(&ConstraintKind::SystemDoctype)
    {
        return false;
    }

    let root_re = regex::Regex::new(r"<(\w+)").unwrap();
    if let Some(m) = root_re.captures(raw) {
        if let Some(pat) = c.get(&ConstraintKind::Root) {
            let root = m.get(1).map(|g| g.as_str()).unwrap_or("");
            if !regex_full(pat, root) {
                return false;
            }
        }
    } else if c.contains_key(&ConstraintKind::Root) {
        return false;
    }

    let xmlns_re = regex::Regex::new(r#"xmlns(?::\w+)?="(.*?)""#).unwrap();
    if let Some(m) = xmlns_re.captures(raw) {
        if let Some(pat) = c.get(&ConstraintKind::Xmlns) {
            let ns = m.get(1).map(|g| g.as_str()).unwrap_or("");
            if !regex_full(pat, ns) {
                return false;
            }
        }
    } else if c.contains_key(&ConstraintKind::Xmlns) {
        return false;
    }
    true
}

fn regex_full(pat: &str, value: &str) -> bool {
    let anchored = if pat.starts_with('^') && pat.ends_with('$') {
        pat.to_string()
    } else {
        format!("^(?:{pat})$")
    };
    regex::Regex::new(&anchored)
        .map(|re| re.is_match(value))
        .unwrap_or(false)
}

/// Match Java `XMLFilter.isFileSupported` against the first 8 KiB.
pub fn file_looks_like(raw: &str, dialect: &dyn XmlDialect) -> bool {
    let limit = raw.char_indices().nth(8192).map(|(i, _)| i).unwrap_or(raw.len());
    is_xml_supported(&raw[..limit], dialect)
}
