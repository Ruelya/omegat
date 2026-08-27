//! Legacy block-oriented XML reader used by preferences and old plugins.
//!
//! Java's deprecated `XMLStreamReader` exposes a stream of open, close,
//! standalone, and text blocks. Keeping this adapter avoids replacing that
//! behavior with a boolean "XML parsed" check.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlBlockKind {
    Open,
    Close,
    Standalone,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlBlock {
    pub kind: XmlBlockKind,
    pub name: String,
    pub text: String,
    pub attributes: BTreeMap<String, String>,
}

impl XmlBlock {
    fn tag(kind: XmlBlockKind, name: String, attributes: BTreeMap<String, String>) -> Self {
        Self {
            kind,
            name,
            text: String::new(),
            attributes,
        }
    }

    fn text(text: String) -> Self {
        Self {
            kind: XmlBlockKind::Text,
            name: String::new(),
            text,
            attributes: BTreeMap::new(),
        }
    }

    /// Stable Java-`XMLBlock` projection used by diagnostics and goldens.
    pub fn descriptor(&self) -> String {
        match self.kind {
            XmlBlockKind::Text => format!("text:{}", self.text),
            XmlBlockKind::Open => format!("open:{}", self.name),
            XmlBlockKind::Close => format!("close:{}", self.name),
            XmlBlockKind::Standalone if self.attributes.is_empty() => {
                format!("standalone:{}", self.name)
            }
            XmlBlockKind::Standalone => {
                let attributes = self
                    .attributes
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("standalone:{}:{attributes}", self.name)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlBlockGroup {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    pub blocks: Vec<XmlBlock>,
}

/// Advance to `target` and return the blocks through its matching close tag.
///
/// Formatting-only text is removed, matching `killEmptyBlocks()`. XML named,
/// decimal, hexadecimal, BMP, and supplementary-plane entities are decoded by
/// the parser; invalid Unicode scalar entities return an error.
pub fn close_block(xml: &str, target: &str) -> Result<XmlBlockGroup, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut found = false;
    let mut depth = 0usize;
    let mut root_attributes = BTreeMap::new();
    let mut blocks = Vec::new();

    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match event {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attributes = attributes(&reader, &e)?;
                if !found {
                    if name == target {
                        found = true;
                        root_attributes = attributes;
                    }
                } else {
                    depth += 1;
                    blocks.push(XmlBlock::tag(XmlBlockKind::Open, name, attributes));
                }
            }
            Event::Empty(e) if found => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attrs = attributes(&reader, &e)?;
                blocks.push(XmlBlock::tag(XmlBlockKind::Standalone, name, attrs));
            }
            Event::End(e) if found => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if depth == 0 && name == target {
                    return Ok(XmlBlockGroup {
                        tag: target.to_string(),
                        attributes: root_attributes,
                        blocks,
                    });
                }
                blocks.push(XmlBlock::tag(XmlBlockKind::Close, name, BTreeMap::new()));
                depth = depth.saturating_sub(1);
            }
            Event::Text(e) if found => {
                let text = e.unescape().map_err(|e| e.to_string())?.into_owned();
                if !text.is_empty() {
                    blocks.push(XmlBlock::text(text));
                }
            }
            Event::CData(e) if found => {
                let text = String::from_utf8_lossy(e.as_ref()).into_owned();
                if !text.is_empty() {
                    blocks.push(XmlBlock::text(text));
                }
            }
            Event::Eof => {
                return Err(if found {
                    format!("unclosed XML block <{target}>")
                } else {
                    format!("XML block <{target}> not found")
                });
            }
            _ => {}
        }
    }
}

fn attributes(
    reader: &Reader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for attr in event.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .decode_and_unescape_value(reader.decoder())
            .map_err(|e| e.to_string())?
            .into_owned();
        out.insert(key, value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{close_block, XmlBlockKind};

    #[test]
    fn decodes_entities_and_keeps_block_kinds() {
        let group = close_block(
            r#"<root><body attr="foo"><a>&#x2603;</a><standalone/></body></root>"#,
            "body",
        )
        .unwrap();
        assert_eq!(group.attributes["attr"], "foo");
        assert_eq!(
            group
                .blocks
                .iter()
                .map(|b| b.kind.clone())
                .collect::<Vec<_>>(),
            vec![
                XmlBlockKind::Open,
                XmlBlockKind::Text,
                XmlBlockKind::Close,
                XmlBlockKind::Standalone
            ]
        );
        assert_eq!(group.blocks[1].text, "☃");
    }

    #[test]
    fn rejects_invalid_scalar_entity() {
        assert!(close_block("<root><body>&#12345678;</body></root>", "body").is_err());
    }
}
