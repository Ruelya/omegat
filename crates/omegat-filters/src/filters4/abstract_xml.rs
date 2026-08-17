//! Java `AbstractXmlFilter` process loop.

use super::stax::{
    detect_eol, detect_xml_encoding, finalize_xml_writer, from_event_to_writer, read_xml_events,
    StaxWriter, XmlEvent,
};
use crate::{ensure_parent, ExtractedSegment, FilterError, Result};
use std::path::Path;

pub trait StaxFilter {
    fn check_cursor(&mut self, ev: &XmlEvent, writing: bool) -> bool;
    fn process_start(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn process_end(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn process_characters(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn take_segments(&mut self) -> Vec<ExtractedSegment>;
}

pub fn process_xml(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    let events = read_xml_events(raw).map_err(|e| FilterError::Parse {
        format: "filters4".into(),
        message: e,
    })?;
    let encoding = detect_xml_encoding(raw);
    let eol = detect_eol(raw);
    let mut writer = StaxWriter::default();
    if writing {
        writer.out.push_str("<?xml version=\"1.0\"?>");
    }
    let mut is_event_mode = false;
    let mut depth: i32 = 0;
    for ev in &events {
        if matches!(ev, XmlEvent::StartDocument { .. } | XmlEvent::EndDocument) {
            continue;
        }
        // JDK StAX does not report prolog/epilog whitespace around the root.
        if depth == 0 {
            if let XmlEvent::Characters { data } | XmlEvent::CData { data } = ev {
                if data.chars().all(|c| c.is_whitespace()) {
                    continue;
                }
            }
        }
        match ev {
            XmlEvent::StartElement { .. } => depth += 1,
            XmlEvent::EndElement { .. } => depth -= 1,
            _ => {}
        }
        if !is_event_mode {
            is_event_mode = filter.check_cursor(ev, writing);
        }
        if is_event_mode {
            let keep = match ev {
                XmlEvent::StartElement { .. } => {
                    if writing {
                        filter.process_start(ev, Some(&mut writer))
                    } else {
                        filter.process_start(ev, None)
                    }
                }
                XmlEvent::EndElement { .. } => {
                    if writing {
                        filter.process_end(ev, Some(&mut writer))
                    } else {
                        filter.process_end(ev, None)
                    }
                }
                XmlEvent::Characters { .. } | XmlEvent::CData { .. } => {
                    if writing {
                        filter.process_characters(ev, Some(&mut writer))
                    } else {
                        filter.process_characters(ev, None)
                    }
                }
                _ => true,
            };
            if writing && keep {
                from_event_to_writer(ev, &mut writer);
            }
        } else if writing {
            from_event_to_writer(ev, &mut writer);
        }
    }
    let segments = filter.take_segments();
    let text = if writing {
        finalize_xml_writer(&writer.out, encoding.as_deref(), &eol)
    } else {
        String::new()
    };
    Ok((segments, text))
}

pub fn parse_xml_file(path: &Path, filter: &mut dyn StaxFilter) -> Result<Vec<ExtractedSegment>> {
    let raw = crate::read_to_string(path)?;
    let (segments, _) = process_xml(&raw, filter, false)?;
    Ok(segments)
}

pub fn write_xml_file(
    source: &Path,
    dest: &Path,
    filter: &mut dyn StaxFilter,
) -> Result<Vec<ExtractedSegment>> {
    let raw = crate::read_to_string(source)?;
    let (segments, text) = process_xml(&raw, filter, true)?;
    ensure_parent(dest)?;
    std::fs::write(dest, text)?;
    Ok(segments)
}

pub fn process_xml_string(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml(raw, filter, writing)
}
