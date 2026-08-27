//! Java `org.omegat.filters4.xml.xliff.Xliff1Filter`.

use super::abstract_xliff::{restore_tags, to_pair, write_events, BufferKind, XliffState};
use super::abstract_xml::{parse_xml_file, write_xml_file, StaxFilter};
use super::stax::{from_event_to_writer, StaxWriter, XmlEvent};
use crate::{Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

const XLIFF12: &str = "urn:oasis:names:tc:xliff:document:1.2";

pub struct Xliff1Filter;

pub struct Xliff1Proc {
    pub xliff: XliffState,
    unit_id: Option<String>,
    flushed_unit: bool,
    last_group_id: i32,
    target_start: Option<XmlEvent>,
    pub standard_state: bool,
    pub event_on_cmt_defs: bool,
    /// Java `AbstractFilter` default `isBilingual()==false` (SdlProject ZIP).
    /// Missing translations fall back to the source text.
    pub fill_missing_with_source: bool,
    error: Option<String>,
}

impl Xliff1Proc {
    pub fn new() -> Self {
        Self {
            xliff: XliffState::new(),
            unit_id: None,
            flushed_unit: false,
            last_group_id: 0,
            target_start: None,
            standard_state: true,
            event_on_cmt_defs: false,
            fill_missing_with_source: false,
            error: None,
        }
    }

    pub fn with_translations(map: &HashMap<String, String>) -> Self {
        let mut s = Self::new();
        s.xliff.set_translations(map);
        s
    }

    fn ns(&self) -> String {
        self.xliff
            .namespace
            .clone()
            .unwrap_or_else(|| XLIFF12.to_string())
    }

    fn required_attr(&mut self, ev: &XmlEvent, name: &str, element: &str) -> Option<String> {
        if let Some(v) = ev.attr(name) {
            return Some(v.to_string());
        }
        self.error = Some(format!("Attribute '{name}' is missing in <{element}>"));
        None
    }

    fn build_tags(&mut self, src_list: &[XmlEvent], reuse: bool) -> String {
        if !reuse {
            self.xliff.tags_map.clear();
            for v in self.xliff.tags_count.values_mut() {
                *v = 0;
            }
        }
        let mut res = String::new();
        let mut native: Option<Vec<XmlEvent>> = None;
        let mut save_buf: Vec<String> = Vec::new();
        for ev in src_list {
            if let Some(n) = &mut native {
                n.push(ev.clone());
            }
            match ev {
                XmlEvent::Characters { data } | XmlEvent::CData { data } => {
                    if native.is_none() {
                        res.push_str(data);
                    }
                }
                XmlEvent::StartElement { name, .. } => {
                    let prefix = self.find_prefix(ev);
                    let count = *self.xliff.tags_count.get(&prefix).unwrap_or(&0);
                    self.xliff.tags_count.insert(prefix, count + 1);
                    match name.local.as_str() {
                        "x" => {
                            res.push_str(&self.xliff.start_pair(
                                reuse,
                                true,
                                ev,
                                'x',
                                count,
                                to_pair(ev),
                                &["rid", "id", "i"],
                            ));
                        }
                        "bx" => {
                            res.push_str(&self.xliff.start_pair(
                                reuse,
                                false,
                                ev,
                                prefix,
                                count,
                                to_pair(ev),
                                &["rid", "id", "i"],
                            ));
                        }
                        "ex" => {
                            res.push_str(&self.xliff.end_pair(
                                reuse,
                                ev,
                                prefix,
                                count,
                                to_pair(ev),
                                &["rid", "id", "i"],
                            ));
                        }
                        "bpt" | "ept" => {
                            native = Some(vec![ev.clone()]);
                            if name.local == "bpt" {
                                res.push_str(&self.xliff.start_pair(
                                    reuse,
                                    false,
                                    ev,
                                    prefix,
                                    count,
                                    Vec::new(),
                                    &["rid", "id", "i"],
                                ));
                            } else {
                                res.push_str(&self.xliff.end_pair(
                                    reuse,
                                    ev,
                                    prefix,
                                    count,
                                    Vec::new(),
                                    &["rid", "id", "i"],
                                ));
                            }
                            save_buf.push(std::mem::take(&mut res));
                        }
                        _ if self.is_protected(ev) => {
                            native = Some(vec![ev.clone()]);
                            if reuse {
                                res.push_str(&super::abstract_xliff::find_key(
                                    &self.xliff.tags_map,
                                    ev,
                                    true,
                                ));
                            } else {
                                let pos = ev.attr("pos").unwrap_or("");
                                if pos == "close" || pos == "end" {
                                    self.xliff
                                        .tags_map
                                        .insert(format!("/{prefix}{count}"), Vec::new());
                                    res.push_str(&format!("</{prefix}{count}>"));
                                } else {
                                    self.xliff
                                        .tags_map
                                        .insert(format!("{prefix}{count}"), Vec::new());
                                    if pos == "open" || pos == "begin" {
                                        res.push_str(&format!("<{prefix}{count}>"));
                                    } else {
                                        res.push_str(&format!("<{prefix}{count}/>"));
                                    }
                                }
                            }
                            save_buf.push(std::mem::take(&mut res));
                            self.xliff.tag_stack.push("mark-protected".into());
                        }
                        _ if self.is_deleted(ev) => {
                            self.xliff.tag_stack.push("mark-deleted".into());
                            save_buf.push(std::mem::take(&mut res));
                        }
                        _ if self.is_untagged(ev) => {
                            self.xliff.tag_stack.push("mark-ignored".into());
                        }
                        _ => {
                            self.xliff
                                .start_stack_element(reuse, ev, prefix, count, &mut res);
                        }
                    }
                }
                XmlEvent::EndElement { name } => match name.local.as_str() {
                    "x" | "bx" | "ex" => {}
                    "bpt" | "ept" => {
                        native = None;
                        res = save_buf.pop().unwrap_or_default();
                    }
                    _ => {
                        let pop = self.xliff.tag_stack.pop().unwrap_or_default();
                        if pop == "mark-protected" {
                            native = None;
                            res = save_buf.pop().unwrap_or_default();
                        } else if pop == "mark-deleted" {
                            res = save_buf.pop().unwrap_or_default();
                        } else if pop != "mark-ignored" && !pop.is_empty() {
                            self.xliff
                                .tags_map
                                .insert(format!("/{pop}"), vec![ev.clone()]);
                            res.push_str(&format!("</{pop}>"));
                        }
                    }
                },
                _ => {}
            }
        }
        res
    }

    fn find_prefix(&self, st: &XmlEvent) -> char {
        let ctype = st.attr("ctype").or_else(|| st.attr("type")).unwrap_or("");
        if !ctype.is_empty() {
            if let Some(rest) = ctype.strip_prefix("x-") {
                return rest.chars().next().unwrap_or('x').to_ascii_lowercase();
            }
            return ctype.chars().next().unwrap_or('x').to_ascii_lowercase();
        }
        let local = st.local_name().unwrap_or("x");
        if local == "bx" || local == "ex" {
            return 'e';
        }
        if local == "bpt" || local == "ept" {
            return 't';
        }
        if local == "it" {
            return 'a';
        }
        if let Some((name, _, _)) = st.as_start() {
            if !name.uri.is_empty() && Some(&name.uri) != self.xliff.namespace.as_ref() {
                return 'o';
            }
        }
        local.chars().next().unwrap_or('x')
    }

    fn is_protected(&self, st: &XmlEvent) -> bool {
        let Some((name, _, _)) = st.as_start() else {
            return false;
        };
        let ns = self.ns();
        (name.local == "ph" && name.uri == ns)
            || (name.local == "it" && name.uri == ns)
            || (name.local == "mrk" && name.uri == ns && st.attr("mtype") == Some("protected"))
    }

    fn is_deleted(&self, _st: &XmlEvent) -> bool {
        false
    }

    fn is_untagged(&self, _st: &XmlEvent) -> bool {
        false
    }

    fn is_standard_state(&self) -> bool {
        self.standard_state
    }

    fn generate_target_start(&self, writer: &mut StaxWriter, translated: bool) {
        let ns = self.ns();
        if !self.is_standard_state() || !translated {
            if let Some(ev) = &self.target_start {
                from_event_to_writer(ev, writer);
            } else {
                writer.write_start_element("", "target", &ns);
            }
            return;
        }
        writer.write_start_element("", "target", &ns);
        writer.write_attribute("", "", "state", "translated");
        if let Some(XmlEvent::StartElement { attrs, .. }) = &self.target_start {
            for a in attrs {
                if a.name.local != "state" {
                    writer.write_attribute(&a.name.prefix, &a.name.uri, &a.name.local, &a.value);
                }
            }
        }
    }

    fn flush_translations(&mut self, writer: Option<&mut StaxWriter>) {
        let Some(writer) = writer else {
            return;
        };
        if self.flushed_unit {
            return;
        }
        let unit_id = match &self.unit_id {
            Some(id) => id.clone(),
            None => return,
        };
        let src = {
            let list = self.xliff.source.clone();
            self.build_tags(&list, false)
        };
        let tra = self
            .xliff
            .lookup_translation(&unit_id, &src, &self.xliff.path)
            .or_else(|| {
                if self.fill_missing_with_source {
                    Some(src.clone())
                } else {
                    None
                }
            });
        if let Some(tra) = tra {
            self.generate_target_start(writer, true);
            let restored = restore_tags(&tra, &self.xliff.tags_map);
            write_events(&restored, writer);
            writer.write_end_element();
            self.flushed_unit = true;
            return;
        }
        if self.xliff.target.is_none() {
            return;
        }
        self.generate_target_start(writer, false);
        if let Some(t) = &self.xliff.target {
            write_events(t, writer);
        }
        writer.write_end_element();
        self.flushed_unit = true;
    }

    fn should_flush_on_start(&self, ev: &XmlEvent) -> bool {
        if !self.xliff.should_register() || self.unit_id.is_none() {
            return false;
        }
        let Some((name, _, _)) = ev.as_start() else {
            return false;
        };
        name.uri != XLIFF12
    }
}

impl StaxFilter for Xliff1Proc {
    fn check_cursor(&mut self, ev: &XmlEvent, _writing: bool) -> bool {
        let Some(local) = ev.local_name() else {
            return false;
        };
        if matches!(ev, XmlEvent::StartElement { .. }) {
            if local == "body"
                || (self.event_on_cmt_defs && (local == "cmt-defs" || local == "tag-defs"))
            {
                return true;
            }
            if local == "xliff" {
                if let Some((name, _, _)) = ev.as_start() {
                    if self.xliff.namespace.is_none() && !name.uri.is_empty() {
                        self.xliff.namespace = Some(name.uri.clone());
                    }
                }
            }
            if local == "file" || local == "group" || local == "unit" {
                let _ = self.process_start(ev, None);
            }
        }
        false
    }

    fn process_start(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool {
        let local = ev.local_name().unwrap_or("");
        match local {
            "xliff" => {
                if let Some((name, _, _)) = ev.as_start() {
                    if self.xliff.namespace.is_none() && !name.uri.is_empty() {
                        self.xliff.namespace = Some(name.uri.clone());
                    }
                }
            }
            "file" => {
                if let Some(orig) = self.required_attr(ev, "original", "file") {
                    self.xliff.path.push('/');
                    self.xliff.path.push_str(&orig);
                }
                self.xliff.update_ignore_scope(ev);
            }
            "group" => {
                if let Some(id) = ev.attr("id") {
                    self.xliff.path.push('/');
                    self.xliff.path.push_str(id);
                } else if let Some(rn) = ev.attr("resname") {
                    self.xliff.path.push('/');
                    self.xliff.path.push_str(rn);
                } else {
                    self.xliff
                        .path
                        .push_str(&format!("/x-auto-{}", self.last_group_id));
                    self.last_group_id += 1;
                }
                self.xliff.update_ignore_scope(ev);
            }
            "trans-unit" => {
                self.unit_id = self.required_attr(ev, "id", "trans-unit");
                self.flushed_unit = false;
                self.target_start = None;
                self.xliff.update_ignore_scope(ev);
            }
            "source" => {
                self.xliff.current = Some(BufferKind::Source);
                self.xliff.source.clear();
            }
            "target" => {
                self.xliff.target = Some(Vec::new());
                self.xliff.current = Some(BufferKind::Target);
                self.xliff.in_target = true;
                self.target_start = Some(ev.clone());
            }
            "note" => {
                self.xliff.current = Some(BufferKind::Note);
                self.xliff.note.clear();
            }
            "seg-source" => {
                self.xliff.current = Some(BufferKind::SegSource);
            }
            _ => {
                if self.xliff.current.is_some() {
                    self.xliff.push_current(ev.clone());
                } else if self.should_flush_on_start(ev) {
                    self.flush_translations(writer);
                }
            }
        }
        !self.xliff.in_target
    }

    fn process_end(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool {
        let local = ev.local_name().unwrap_or("");
        match local {
            "source" | "seg-source" | "note" => {
                self.xliff.current = None;
            }
            "target" => {
                self.xliff.current = None;
                if self.xliff.should_register() {
                    self.flush_translations(writer);
                }
                self.xliff.in_target = false;
                return false;
            }
            "trans-unit" => {
                if self.xliff.should_register() {
                    self.flush_translations(writer);
                    let id = self.unit_id.clone().unwrap_or_default();
                    let source = self.xliff.source.clone();
                    let target = self.xliff.target.clone();
                    let src = self.build_tags(&source, false);
                    let tra = target
                        .as_ref()
                        .filter(|t| !t.is_empty())
                        .map(|t| self.build_tags(t, true));
                    if !src.is_empty() {
                        let note = if self.xliff.note.is_empty() {
                            None
                        } else {
                            Some(
                                self.xliff
                                    .note
                                    .iter()
                                    .filter_map(|e| match e {
                                        XmlEvent::Characters { data }
                                        | XmlEvent::CData { data } => Some(data.as_str()),
                                        _ => None,
                                    })
                                    .collect::<String>(),
                            )
                        };
                        self.xliff.segments.push(crate::ExtractedSegment {
                            id,
                            source: src,
                            existing_translation: tra,
                            note: note.clone(),
                            comment: note,
                            path: Some(self.xliff.path.clone()),
                            protected_parts: vec![],
                        });
                    }
                }
                self.unit_id = None;
                self.xliff.clean_buffers();
                self.xliff.pop_ignore_scope(local);
            }
            "file" => {
                self.xliff.path = "/".into();
                self.xliff.clean_buffers();
                self.xliff.pop_ignore_scope(local);
            }
            "group" => {
                if let Some(idx) = self.xliff.path.rfind('/') {
                    self.xliff.path.truncate(idx);
                }
                self.xliff.clean_buffers();
                self.xliff.pop_ignore_scope(local);
            }
            _ => {
                if self.xliff.current.is_some() {
                    self.xliff.push_current(ev.clone());
                }
            }
        }
        !self.xliff.in_target
    }

    fn process_characters(&mut self, ev: &XmlEvent, _writer: Option<&mut StaxWriter>) -> bool {
        if self.xliff.current.is_some() {
            self.xliff.push_current(ev.clone());
        }
        !self.xliff.in_target
    }

    fn take_segments(&mut self) -> Vec<crate::ExtractedSegment> {
        std::mem::take(&mut self.xliff.segments)
    }

    fn fatal_error(&self) -> Option<String> {
        self.error.clone()
    }
}

impl Filter for Xliff1Filter {
    fn id(&self) -> &'static str {
        "xliff1"
    }
    fn name(&self) -> &'static str {
        "XLIFF 1"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xlf", "*.xliff"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let mut proc = Xliff1Proc::new();
        let segments = parse_xml_file(path, &mut proc)?;
        Ok(ParsedFile {
            segments,
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let mut proc = Xliff1Proc::with_translations(translations);
        write_xml_file(source_path, dest_path, &mut proc)?;
        Ok(())
    }
}
