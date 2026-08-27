//! Java `org.omegat.filters2.html2` — FilterVisitor + HTMLOptions + HTMLWriter.

mod filter_visitor;
mod html_options;
mod html_writer;
mod tokenizer;

pub use filter_visitor::{process_html, VisitorKind};
pub use html_writer::entities_to_chars;

use crate::{ensure_parent, Filter, FilterContext, ParsedFile, Result};
use encoding_rs::Encoding;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];
const UTF16LE_BOM: &[u8] = &[0xff, 0xfe];
const UTF16BE_BOM: &[u8] = &[0xfe, 0xff];

struct DecodedHtml {
    text: String,
    encoding: &'static Encoding,
    bom: &'static [u8],
}

fn declared_html_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    // Encoding declarations are ASCII-compatible. Mirroring Java's sniffer,
    // only inspect the beginning of the document and prefer the XML header.
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(8192)]);
    static XML_ENCODING: OnceLock<Regex> = OnceLock::new();
    let xml = XML_ENCODING.get_or_init(|| {
        Regex::new(r#"(?i)<\?xml[^>]*\bencoding\s*=\s*["']?\s*([^"'\s?>]+)"#).unwrap()
    });
    static META_ENCODING: OnceLock<Regex> = OnceLock::new();
    let meta = META_ENCODING.get_or_init(|| {
        Regex::new(r#"(?i)<meta\b[^>]*\bcharset\s*=\s*["']?\s*([^"'\s/>;]+)"#).unwrap()
    });
    [xml, meta].into_iter().find_map(|pattern| {
        let label = pattern.captures(&prefix)?.get(1)?.as_str();
        Encoding::for_label(label.as_bytes())
    })
}

fn decode_html_bytes_with_fallback(bytes: &[u8], fallback: Option<&str>) -> DecodedHtml {
    let (encoding, bom, payload) = if bytes.starts_with(UTF8_BOM) {
        (encoding_rs::UTF_8, UTF8_BOM, &bytes[UTF8_BOM.len()..])
    } else if bytes.starts_with(UTF16LE_BOM) {
        (
            encoding_rs::UTF_16LE,
            UTF16LE_BOM,
            &bytes[UTF16LE_BOM.len()..],
        )
    } else if bytes.starts_with(UTF16BE_BOM) {
        (
            encoding_rs::UTF_16BE,
            UTF16BE_BOM,
            &bytes[UTF16BE_BOM.len()..],
        )
    } else {
        let encoding = declared_html_encoding(bytes)
            .or_else(|| fallback.and_then(|label| Encoding::for_label(label.as_bytes())))
            .unwrap_or_else(|| {
                if std::str::from_utf8(bytes).is_ok() {
                    encoding_rs::UTF_8
                } else {
                    encoding_rs::WINDOWS_1252
                }
            });
        (encoding, &[][..], bytes)
    };
    let (text, _, _) = encoding.decode(payload);
    DecodedHtml {
        text: text.into_owned(),
        encoding,
        bom,
    }
}

#[cfg(test)]
fn decode_html_bytes(bytes: &[u8]) -> DecodedHtml {
    decode_html_bytes_with_fallback(bytes, None)
}

fn read_html(path: &Path, fallback: Option<&str>) -> Result<DecodedHtml> {
    Ok(decode_html_bytes_with_fallback(
        &std::fs::read(path)?,
        fallback,
    ))
}

fn encode_html(text: &str, encoding: &'static Encoding, bom: &[u8]) -> Vec<u8> {
    let body = if encoding == encoding_rs::UTF_16LE {
        text.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    } else if encoding == encoding_rs::UTF_16BE {
        text.encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>()
    } else {
        let (bytes, _, _) = encoding.encode(text);
        bytes.into_owned()
    };
    let mut encoded = Vec::with_capacity(bom.len() + body.len());
    encoded.extend_from_slice(bom);
    encoded.extend_from_slice(&body);
    encoded
}

pub struct HtmlFilter;

impl Filter for HtmlFilter {
    fn id(&self) -> &'static str {
        "html"
    }
    fn name(&self) -> &'static str {
        "HTML and XHTML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.html", "*.htm", "*.xhtml", "*.xht"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let source = read_html(path, ctx.in_encoding.as_deref())?;
        Ok(filter_visitor::process_html_with_encoding(
            &source.text,
            ctx,
            VisitorKind::Html,
            None,
            source.encoding.name(),
        )
        .parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let source = read_html(source_path, ctx.in_encoding.as_deref())?;
        let target_encoding = ctx
            .out_encoding
            .as_deref()
            .and_then(|label| Encoding::for_label(label.as_bytes()))
            .unwrap_or(source.encoding);
        let target_bom = if ctx.out_encoding.is_none() {
            source.bom
        } else {
            &[]
        };
        let out = filter_visitor::process_html_with_encoding(
            &source.text,
            ctx,
            VisitorKind::Html,
            Some(translations),
            target_encoding.name(),
        )
        .written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, encode_html(&out, target_encoding, target_bom))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn utf16_bytes(text: &str, little_endian: bool) -> Vec<u8> {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in text.encode_utf16() {
            let encoded = if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            };
            bytes.extend_from_slice(&encoded);
        }
        bytes
    }

    #[test]
    fn html_filter_reads_utf16_bom_boundaries() {
        let dir = tempdir().unwrap();
        for (name, little_endian) in [("little.html", true), ("big.html", false)] {
            let path = dir.path().join(name);
            std::fs::write(
                &path,
                utf16_bytes("<html><body><p>Zażółć 😀</p></body></html>", little_endian),
            )
            .unwrap();
            let parsed = HtmlFilter.parse(&path, &FilterContext::default()).unwrap();
            let sources: Vec<_> = parsed
                .segments
                .iter()
                .map(|segment| segment.source.as_str())
                .collect();
            assert_eq!(sources, vec!["Zażółć 😀"], "{name}");
        }
    }

    #[test]
    fn html_filter_writes_utf16_in_the_original_encoding_with_bom() {
        let dir = tempdir().unwrap();
        let mut ctx = FilterContext::default();
        ctx.options.insert("rewriteEncoding".into(), "NEVER".into());
        for (name, little_endian, encoding) in [
            ("little.html", true, "UTF-16LE"),
            ("big.html", false, "UTF-16BE"),
        ] {
            let source = dir.path().join(name);
            let target = dir.path().join(format!("out-{name}"));
            let html = format!(
                r#"<html><head><meta charset="{encoding}"></head><body><p>Hello 😀</p></body></html>"#
            );
            std::fs::write(&source, utf16_bytes(&html, little_endian)).unwrap();
            HtmlFilter
                .write(
                    &source,
                    &target,
                    &HashMap::from([("Hello 😀".into(), "Bonjour 🦀".into())]),
                    &ctx,
                )
                .unwrap();

            let bytes = std::fs::read(&target).unwrap();
            assert_eq!(
                bytes.starts_with(if little_endian {
                    UTF16LE_BOM
                } else {
                    UTF16BE_BOM
                }),
                true,
                "{name}"
            );
            let decoded = decode_html_bytes(&bytes);
            assert_eq!(decoded.encoding.name(), encoding, "{name}");
            assert_eq!(
                decoded.text,
                format!(
                    r#"<html><head><meta charset="{encoding}"></head><body><p>Bonjour 🦀</p></body></html>"#
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn html_filter_writes_declared_legacy_encoding_without_utf8_conversion() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("legacy.html");
        let target = dir.path().join("legacy-out.html");
        let html =
            r#"<html><head><meta charset="windows-1252"></head><body><p>café</p></body></html>"#;
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(html);
        std::fs::write(&source, bytes.as_ref()).unwrap();
        let mut ctx = FilterContext::default();
        ctx.options.insert("rewriteEncoding".into(), "NEVER".into());

        HtmlFilter
            .write(
                &source,
                &target,
                &HashMap::from([("café".into(), "été".into())]),
                &ctx,
            )
            .unwrap();

        let bytes = std::fs::read(&target).unwrap();
        assert_eq!(std::str::from_utf8(&bytes).is_err(), true);
        let decoded = decode_html_bytes(&bytes);
        assert_eq!(decoded.encoding.name(), "windows-1252");
        assert_eq!(
            decoded.text,
            r#"<html><head><meta charset="windows-1252"></head><body><p>été</p></body></html>"#
        );
    }

    #[test]
    fn html_filter_explicit_target_encoding_overrides_detected_source() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("legacy.html");
        let target = dir.path().join("utf8.html");
        let html =
            r#"<html><head><meta charset="windows-1252"></head><body><p>café</p></body></html>"#;
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(html);
        std::fs::write(&source, bytes.as_ref()).unwrap();
        let mut ctx = FilterContext::default();
        ctx.out_encoding = Some("UTF-8".into());
        ctx.options
            .insert("rewriteEncoding".into(), "ALWAYS".into());

        HtmlFilter
            .write(&source, &target, &HashMap::new(), &ctx)
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            r#"<html><head><meta charset="UTF-8"></head><body><p>café</p></body></html>"#
        );
    }

    #[test]
    fn html_filter_recovers_after_incomplete_markup() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("broken.html");
        let target = dir.path().join("translated.html");
        let raw = "<broken attribute <p>Hello</p><?unfinished";
        std::fs::write(&source, raw).unwrap();

        let parsed = HtmlFilter
            .parse(&source, &FilterContext::default())
            .unwrap();
        assert_eq!(
            parsed
                .segments
                .iter()
                .map(|segment| segment.source.as_str())
                .collect::<Vec<_>>(),
            vec!["Hello"]
        );

        HtmlFilter
            .write(
                &source,
                &target,
                &HashMap::from([("Hello".into(), "Bonjour".into())]),
                &FilterContext::default(),
            )
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(target).unwrap(),
            "<broken attribute <p>Bonjour</p><?unfinished"
        );
    }
}
