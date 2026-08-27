//! Shared bilingual XLIFF state — Java `AbstractXliffFilter`.

use super::stax::{from_event_to_writer, StaxWriter, XmlEvent};
use crate::{ExtractedSegment, ProtectedPart};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub static OMEGAT_TAG: OnceLock<Regex> = OnceLock::new();

pub fn omegat_tag() -> &'static Regex {
    OMEGAT_TAG.get_or_init(|| Regex::new(r"<(/?)([a-zA-Z]\d+)/?>").unwrap())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferKind {
    Source,
    Target,
    Note,
    SegSource,
}

#[derive(Default)]
pub struct XliffState {
    pub namespace: Option<String>,
    pub path: String,
    pub ignore_scope: Option<String>,
    pub current: Option<BufferKind>,
    pub in_target: bool,
    pub source: Vec<XmlEvent>,
    pub target: Option<Vec<XmlEvent>>,
    pub note: Vec<XmlEvent>,
    pub tags_map: BTreeMap<String, Vec<XmlEvent>>,
    pub tags_count: BTreeMap<char, i32>,
    pub tag_stack: Vec<String>,
    pub paired_holders: BTreeMap<String, String>,
    pub segments: Vec<ExtractedSegment>,
    pub translations: BTreeMap<String, String>,
}

impl XliffState {
    pub fn new() -> Self {
        Self {
            path: "/".into(),
            ..Self::default()
        }
    }

    pub fn set_translations(&mut self, map: &std::collections::HashMap<String, String>) {
        self.translations.clear();
        for (k, v) in map {
            if !v.is_empty() {
                self.translations.insert(k.clone(), v.clone());
            }
        }
    }

    pub fn clean_buffers(&mut self) {
        self.source.clear();
        self.target = None;
        self.note.clear();
        self.current = None;
    }

    pub fn push_current(&mut self, ev: XmlEvent) {
        match self.current {
            Some(BufferKind::Source) => self.source.push(ev),
            Some(BufferKind::Target) => {
                if let Some(t) = &mut self.target {
                    t.push(ev);
                }
            }
            Some(BufferKind::Note) => self.note.push(ev),
            Some(BufferKind::SegSource) => {}
            None => {}
        }
    }

    pub fn update_ignore_scope(&mut self, ev: &XmlEvent) {
        let Some(val) = ev.attr("translate") else {
            return;
        };
        let local = ev.local_name().unwrap_or("");
        if val == "no" {
            self.ignore_scope = Some(local.to_string());
        } else if val == "yes" {
            if let Some(old) = self.ignore_scope.take() {
                self.ignore_scope = Some(format!("!{local} {old}"));
            }
        }
    }

    pub fn pop_ignore_scope(&mut self, local: &str) {
        match self.ignore_scope.as_deref() {
            Some(s) if s == local => self.ignore_scope = None,
            Some(s) if s.starts_with(&format!("!{local}")) => {
                let skip = local.len() + 2;
                self.ignore_scope = Some(s[skip.min(s.len())..].to_string());
            }
            _ => {}
        }
    }

    pub fn should_register(&self) -> bool {
        self.ignore_scope
            .as_deref()
            .map(|s| s.starts_with('!'))
            .unwrap_or(true)
    }

    pub fn lookup_translation(&self, id: &str, source: &str, path: &str) -> Option<String> {
        // Java `ITranslateCallback.getTranslation(id, source, path)` keys on
        // source text. A bare id match would apply one unit's translation to
        // every other unit that reused the same numeric id (XLIFF 2 `1`).
        if let Some(t) = self.translations.get(source) {
            return Some(t.clone());
        }
        if !id.is_empty() {
            let key = format!("{id}\t{source}");
            if let Some(t) = self.translations.get(&key) {
                return Some(t.clone());
            }
            let key = format!("{id}\t{path}");
            if let Some(t) = self.translations.get(&key) {
                return Some(t.clone());
            }
        }
        None
    }

    pub fn register_unit(
        &mut self,
        entry_id: &str,
        unit_source: &[XmlEvent],
        unit_target: Option<&[XmlEvent]>,
        build_tags: impl Fn(&mut Self, &[XmlEvent], bool) -> String,
    ) {
        let src = build_tags(self, unit_source, false);
        let tra = unit_target
            .filter(|t| !t.is_empty())
            .map(|t| build_tags(self, t, true));
        if src.is_empty() {
            return;
        }
        let note = if self.note.is_empty() {
            None
        } else {
            Some(note_text(&self.note))
        };
        let protected = build_protected_parts(&src, &self.tags_map);
        self.segments.push(ExtractedSegment {
            id: entry_id.to_string(),
            source: src,
            existing_translation: tra,
            note: note.clone(),
            comment: note,
            path: Some(self.path.clone()),
            protected_parts: protected,
        });
    }

    pub fn start_pair(
        &mut self,
        reuse: bool,
        is_empty: bool,
        st: &XmlEvent,
        prefix: char,
        count: i32,
        native: Vec<XmlEvent>,
        pair_id_names: &[&str],
    ) -> String {
        if reuse {
            return find_key(&self.tags_map, st, is_empty);
        }
        self.tags_map.insert(format!("{prefix}{count}"), native);
        if !is_empty {
            if let Some(id) = pair_id_names.iter().find_map(|n| st.attr(n)) {
                self.paired_holders
                    .insert(id.to_string(), format!("{prefix}{count}"));
            }
        }
        if is_empty {
            format!("<{prefix}{count}/>")
        } else {
            format!("<{prefix}{count}>")
        }
    }

    pub fn end_pair(
        &mut self,
        reuse: bool,
        st: &XmlEvent,
        prefix: char,
        count: i32,
        native: Vec<XmlEvent>,
        pair_id_names: &[&str],
    ) -> String {
        self.tags_count.insert(prefix, count);
        let pair_id = pair_id_names.iter().find_map(|n| st.attr(n));
        let key = pair_id
            .and_then(|id| self.paired_holders.get(id).cloned())
            .unwrap_or_else(|| format!("{prefix}{count}"));
        if !reuse {
            self.tags_map.insert(format!("/{key}"), native);
        }
        format!("</{key}>")
    }

    pub fn start_stack_element(
        &mut self,
        reuse: bool,
        st: &XmlEvent,
        prefix: char,
        count: i32,
        res: &mut String,
    ) {
        if reuse {
            let k = find_key(&self.tags_map, st, false);
            if let Some(c) = omegat_tag().captures(&k) {
                self.tag_stack.push(c[2].to_string());
                res.push_str(&k);
            } else {
                self.tag_stack.push(format!("z{count}"));
                res.push_str(&format!("<z{count}>"));
            }
        } else {
            self.tags_map
                .insert(format!("{prefix}{count}"), vec![st.clone()]);
            res.push_str(&format!("<{prefix}{count}>"));
            self.tag_stack.push(format!("{prefix}{count}"));
        }
    }
}

pub fn find_key(
    tags_map: &BTreeMap<String, Vec<XmlEvent>>,
    find_el: &XmlEvent,
    is_empty: bool,
) -> String {
    let Some((find_name, find_attrs, _)) = find_el.as_start() else {
        return String::new();
    };
    for (key, evs) in tags_map {
        let Some(first) = evs.first() else {
            continue;
        };
        let Some((map_name, map_attrs, _)) = first.as_start() else {
            continue;
        };
        if map_name.local != find_name.local || map_name.uri != find_name.uri {
            continue;
        }
        let mut diff = false;
        for a in map_attrs {
            let found = find_attrs.iter().find(|b| b.name.local == a.name.local);
            match found {
                None => diff = true,
                Some(b) if b.value != a.value => diff = true,
                _ => {}
            }
        }
        if !diff {
            return if is_empty {
                format!("<{key}/>")
            } else {
                format!("<{key}>")
            };
        }
    }
    String::new()
}

pub fn restore_tags(tra: &str, tags_map: &BTreeMap<String, Vec<XmlEvent>>) -> Vec<XmlEvent> {
    let mut res = Vec::new();
    let mut rest = tra;
    let re = omegat_tag();
    while !rest.is_empty() {
        if let Some(m) = re.find(rest) {
            if m.start() > 0 {
                res.push(XmlEvent::Characters {
                    data: rest[..m.start()].to_string(),
                });
            }
            let cap = re.captures(m.as_str()).unwrap();
            let key = format!("{}{}", &cap[1], &cap[2]);
            if let Some(saved) = tags_map.get(&key) {
                res.extend(saved.iter().cloned());
            }
            rest = &rest[m.end()..];
        } else {
            res.push(XmlEvent::Characters {
                data: rest.to_string(),
            });
            break;
        }
    }
    res
}

pub fn write_events(events: &[XmlEvent], writer: &mut StaxWriter) {
    for ev in events {
        from_event_to_writer(ev, writer);
    }
}

pub fn to_pair(st: &XmlEvent) -> Vec<XmlEvent> {
    let Some((name, _, _)) = st.as_start() else {
        return vec![st.clone()];
    };
    vec![st.clone(), XmlEvent::EndElement { name: name.clone() }]
}

fn note_text(note: &[XmlEvent]) -> String {
    let mut s = String::new();
    for ev in note {
        match ev {
            XmlEvent::Characters { data } | XmlEvent::CData { data } => s.push_str(data),
            _ => {}
        }
    }
    s
}

fn build_protected_parts(
    src: &str,
    tags_map: &BTreeMap<String, Vec<XmlEvent>>,
) -> Vec<ProtectedPart> {
    let mut out = Vec::new();
    let mut rest = src;
    let re = omegat_tag();
    while let Some(m) = re.find(rest) {
        let cap = re.captures(m.as_str()).unwrap();
        let key = format!("{}{}", &cap[1], &cap[2]);
        if tags_map.contains_key(&key) {
            out.push(ProtectedPart {
                text: m.as_str().to_string(),
                details: "tag".into(),
            });
        }
        rest = &rest[m.end()..];
    }
    out
}
