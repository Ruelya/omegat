//! Java `org.omegat.filters4.xml.xliff.Xliff2Filter`.

use super::abstract_xliff::{restore_tags, to_pair, write_events, BufferKind, XliffState};
use super::abstract_xml::{parse_xml_file, write_xml_file, StaxFilter};
use super::stax::{StaxWriter, XmlEvent};
use crate::{ExtractedSegment, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Xliff2Filter;

pub struct Xliff2Proc {
    pub xliff: XliffState,
    seg_id: Option<String>,
    flushed_segment: bool,
}

impl Xliff2Proc {
    pub fn new() -> Self {
        Self {
            xliff: XliffState::new(),
            seg_id: None,
            flushed_segment: false,
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
            .unwrap_or_else(|| "urn:oasis:names:tc:xliff:document:2.0".into())
    }

    fn build_tags(&mut self, src_list: &[XmlEvent], reuse: bool) -> String {
        if !reuse {
            self.xliff.tags_map.clear();
            for v in self.xliff.tags_count.values_mut() {
                *v = 0;
            }
        }
        let mut res = String::new();
        for ev in src_list {
            match ev {
                XmlEvent::Characters { data } | XmlEvent::CData { data } => res.push_str(data),
                XmlEvent::StartElement { name, .. } => {
                    let prefix = self.find_prefix(ev);
                    let mut count = *self.xliff.tags_count.get(&prefix).unwrap_or(&0);
                    // Java only increments when the prefix was absent.
                    if !self.xliff.tags_count.contains_key(&prefix) {
                        self.xliff.tags_count.insert(prefix, count + 1);
                        count = 0;
                    }
                    match name.local.as_str() {
                        "mrk" => {}
                        "ph" | "cp" => {
                            res.push_str(&self.xliff.start_pair(
                                reuse,
                                true,
                                ev,
                                prefix,
                                count,
                                to_pair(ev),
                                &["id"],
                            ));
                        }
                        "sc" | "sm" => {
                            res.push_str(&self.xliff.start_pair(
                                reuse,
                                false,
                                ev,
                                prefix,
                                count,
                                to_pair(ev),
                                &["id"],
                            ));
                        }
                        "ec" | "em" => {
                            res.push_str(&self.xliff.end_pair(
                                reuse,
                                ev,
                                prefix,
                                count,
                                to_pair(ev),
                                &["startRef", "id"],
                            ));
                        }
                        _ => {
                            self.xliff
                                .start_stack_element(reuse, ev, prefix, count, &mut res);
                        }
                    }
                }
                XmlEvent::EndElement { name } => match name.local.as_str() {
                    "mrk" | "ph" | "cp" | "sc" | "ec" => {}
                    _ => {
                        let pop = self.xliff.tag_stack.pop().unwrap_or_default();
                        if !pop.is_empty() {
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
        if let Some(ty) = st.attr("type") {
            if ty == "fmt" {
                if let Some(sub) = st.attr("subType") {
                    if let Some(rest) = sub.strip_prefix("xlf:") {
                        return rest.chars().next().unwrap_or('f');
                    }
                }
                return 'f';
            }
        }
        let local = st.local_name().unwrap_or("x");
        if local == "pc" {
            return 'g';
        }
        if local == "sc" || local == "ec" {
            return 't';
        }
        if local == "sm" || local == "em" {
            return 'a';
        }
        if let Some((name, _, _)) = st.as_start() {
            if !name.uri.is_empty() && Some(&name.uri) != self.xliff.namespace.as_ref() {
                return 'o';
            }
        }
        local.chars().next().unwrap_or('x')
    }

    fn flush_translations(&mut self, writer: Option<&mut StaxWriter>) {
        let Some(writer) = writer else {
            return;
        };
        if self.flushed_segment {
            return;
        }
        let seg = self.seg_id.clone().unwrap_or_default();
        let src = {
            let list = self.xliff.source.clone();
            self.build_tags(&list, false)
        };
        let tra = self.xliff.lookup_translation(&seg, &src, &self.xliff.path);
        let ns = self.ns();
        if let Some(tra) = tra {
            writer.write_start_element("", "target", &ns);
            let restored = restore_tags(&tra, &self.xliff.tags_map);
            write_events(&restored, writer);
            writer.write_end_element();
            self.flushed_segment = true;
            return;
        }
        if self.xliff.target.is_none() {
            return;
        }
        writer.write_start_element("", "target", &ns);
        if let Some(t) = &self.xliff.target {
            write_events(t, writer);
        }
        writer.write_end_element();
        self.flushed_segment = true;
    }

    fn register_seg(&mut self) {
        if !self.xliff.should_register() {
            return;
        }
        let id = self.seg_id.clone().unwrap_or_default();
        let source = self.xliff.source.clone();
        let target = self.xliff.target.clone();
        let src = self.build_tags(&source, false);
        let tra = target
            .as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| self.build_tags(t, true));
        if src.is_empty() {
            return;
        }
        self.xliff.segments.push(ExtractedSegment {
            id,
            source: src,
            existing_translation: tra,
            note: None,
            comment: None,
            path: Some(self.xliff.path.clone()),
            protected_parts: vec![],
        });
    }
}

impl StaxFilter for Xliff2Proc {
    fn check_cursor(&mut self, ev: &XmlEvent, _writing: bool) -> bool {
        if matches!(ev, XmlEvent::StartElement { .. }) && ev.local_name() == Some("xliff") {
            if let Some((name, _, _)) = ev.as_start() {
                if self.xliff.namespace.is_none() && !name.uri.is_empty() {
                    self.xliff.namespace = Some(name.uri.clone());
                }
            }
            return true;
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
            "file" | "group" | "unit" => {
                if let Some(id) = ev.attr("id") {
                    self.xliff.path.push('/');
                    self.xliff.path.push_str(id);
                }
                self.xliff.update_ignore_scope(ev);
            }
            "segment" => {
                if let Some(id) = ev.attr("id") {
                    self.seg_id = Some(id.to_string());
                } else if self.seg_id.as_deref().map(|s| s.chars().all(|c| c == '-' || c.is_ascii_digit())).unwrap_or(false)
                {
                    let n: i32 = self.seg_id.as_deref().unwrap_or("0").parse().unwrap_or(0);
                    self.seg_id = Some((n + 1).to_string());
                } else {
                    self.seg_id = Some("1".into());
                }
                self.flushed_segment = false;
            }
            "source" => {
                self.xliff.current = Some(BufferKind::Source);
                self.xliff.source.clear();
            }
            "target" => {
                self.xliff.target = Some(Vec::new());
                self.xliff.current = Some(BufferKind::Target);
                self.xliff.in_target = true;
            }
            "notes" => self.xliff.note.clear(),
            "note" => {
                self.xliff.current = Some(BufferKind::Note);
            }
            _ => {
                if self.xliff.current.is_some() {
                    self.xliff.push_current(ev.clone());
                } else if self.xliff.should_register() && self.seg_id.is_some() {
                    if let Some((name, _, _)) = ev.as_start() {
                        if Some(&name.uri) != self.xliff.namespace.as_ref() {
                            self.flush_translations(writer);
                        }
                    }
                }
            }
        }
        !self.xliff.in_target
    }

    fn process_end(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool {
        let local = ev.local_name().unwrap_or("");
        match local {
            "source" | "note" => self.xliff.current = None,
            "target" => {
                self.xliff.current = None;
                if self.xliff.should_register() {
                    self.flush_translations(writer);
                }
                self.xliff.in_target = false;
                return false;
            }
            "segment" => {
                if self.xliff.should_register() {
                    self.flush_translations(writer);
                    self.register_seg();
                }
                self.seg_id = None;
                self.xliff.clean_buffers();
            }
            "unit" | "group" | "file" => {
                self.seg_id = Some(String::new());
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

    fn take_segments(&mut self) -> Vec<ExtractedSegment> {
        std::mem::take(&mut self.xliff.segments)
    }
}

impl Filter for Xliff2Filter {
    fn id(&self) -> &'static str {
        "xliff2"
    }
    fn name(&self) -> &'static str {
        "XLIFF 2"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xlf", "*.xliff"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn matches(&self, path: &Path) -> bool {
        if !Xliff2Filter.default_masks().iter().any(|m| {
            path.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.to_ascii_lowercase().ends_with(m.trim_start_matches('*')))
                .unwrap_or(false)
        }) {
            return false;
        }
        crate::read_to_string(path)
            .map(|s| s.contains("urn:oasis:names:tc:xliff:document:2.0") || s.contains("version=\"2."))
            .unwrap_or(false)
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let mut proc = Xliff2Proc::new();
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
        let mut proc = Xliff2Proc::with_translations(translations);
        write_xml_file(source_path, dest_path, &mut proc)?;
        Ok(())
    }
}
