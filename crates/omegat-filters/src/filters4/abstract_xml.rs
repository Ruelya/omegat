//! Java `AbstractXmlFilter` process loop.

use super::stax::{
    detect_eol, detect_xml_encoding, detect_xml_standalone, finalize_xml_writer_ex,
    from_event_to_writer, java_xml_declaration, read_xml_events_cancellable, StaxWriter,
    XmlDeclStyle, XmlEvent,
};
use crate::{ensure_parent, ExtractedSegment, FilterError, Result};
use std::path::Path;

pub trait StaxFilter {
    fn check_cursor(&mut self, ev: &XmlEvent, writing: bool) -> bool;
    fn process_start(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn process_end(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn process_characters(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool;
    fn take_segments(&mut self) -> Vec<ExtractedSegment>;
    /// Java `XMLStreamException` / `TranslationException` from a required attribute.
    fn fatal_error(&self) -> Option<String> {
        None
    }
}

pub fn process_xml(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml_cancellable(raw, filter, writing, &|| false)
}

pub fn process_xml_cancellable(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml_ex_cancellable(
        raw,
        filter,
        writing,
        XmlDeclStyle::Woodstox,
        is_cancelled,
    )
}

/// `decl` selects Java `AbstractXmlFilter` prolog vs Woodstox `writeStartDocument`.
pub fn process_xml_ex(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
    decl: XmlDeclStyle,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml_ex_cancellable(raw, filter, writing, decl, &|| false)
}

pub fn process_xml_ex_cancellable(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
    decl: XmlDeclStyle,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let events = read_xml_events_cancellable(raw, is_cancelled).map_err(|e| {
        if e == crate::xml_engine::CANCELLED_ERROR || is_cancelled() {
            FilterError::Cancelled
        } else {
            FilterError::Parse {
                format: "filters4".into(),
                message: e,
            }
        }
    })?;
    let encoding = detect_xml_encoding(raw);
    let standalone = detect_xml_standalone(raw);
    let eol = detect_eol(raw);
    let mut writer = StaxWriter::default();
    if writing {
        writer.out.push_str(&java_xml_declaration(
            Some("1.0"),
            encoding.as_deref(),
            standalone.as_deref(),
            decl,
        ));
    }
    let mut is_event_mode = false;
    let mut depth: i32 = 0;
    for ev in &events {
        if is_cancelled() {
            return Err(FilterError::Cancelled);
        }
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
        if let Some(msg) = filter.fatal_error() {
            return Err(FilterError::Parse {
                format: "filters4".into(),
                message: msg,
            });
        }
    }
    if writing {
        writer.close_remaining();
    }
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
    let segments = filter.take_segments();
    let text = if writing {
        finalize_xml_writer_ex(
            &writer.out,
            encoding.as_deref(),
            standalone.as_deref(),
            &eol,
            decl,
        )
    } else {
        String::new()
    };
    if is_cancelled() {
        return Err(FilterError::Cancelled);
    }
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

pub fn process_xml_string_ex(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
    decl: XmlDeclStyle,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml_ex(raw, filter, writing, decl)
}

pub fn process_xml_string_ex_cancellable(
    raw: &str,
    filter: &mut dyn StaxFilter,
    writing: bool,
    decl: XmlDeclStyle,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<ExtractedSegment>, String)> {
    process_xml_ex_cancellable(raw, filter, writing, decl, is_cancelled)
}
