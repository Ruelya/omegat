use crate::{
    ensure_parent, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

/// Extracts visible strings from a PDF by scanning literal `(...)` text objects.
/// This is a bounded rewrite of OmegaT's PDFBox-based filter (see STATUS.md).
pub struct PdfFilter;

impl Filter for PdfFilter {
    fn id(&self) -> &'static str {
        "pdf"
    }
    fn name(&self) -> &'static str {
        "PDF text contents"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.pdf"]
    }
    fn phase(&self) -> u8 {
        4
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let bytes = std::fs::read(path)?;
        let text = extract_pdf_strings(&bytes);
        let mut segments = Vec::new();
        for chunk in text.split("\n\n") {
            let t = chunk.trim();
            if t.is_empty() {
                continue;
            }
            segments.push(ExtractedSegment {
                id: segments.len().to_string(),
                source: t.to_string(),
                existing_translation: None,
                note: Some("PDF compile writes a sidecar .txt (no binary rewrite)".into()),
                comment: None,
                path: None,
                protected_parts: vec![],
            });
        }
        Ok(ParsedFile {
            segments,
            skeleton: Some(text),
        })
    }
    fn write(
        &self,
        _source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let mut lines: Vec<(usize, String)> = translations
            .iter()
            .filter_map(|(k, v)| k.parse::<usize>().ok().map(|i| (i, v.clone())))
            .collect();
        lines.sort_by_key(|(i, _)| *i);
        let body = if lines.is_empty() {
            translations.values().cloned().collect::<Vec<_>>().join("\n\n")
        } else {
            lines.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n\n")
        };
        let dest = if dest_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            dest_path.with_extension("pdf.txt")
        } else {
            dest_path.to_path_buf()
        };
        ensure_parent(&dest)?;
        std::fs::write(dest, body)?;
        Ok(())
    }
}

fn extract_pdf_strings(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            i += 1;
            let mut buf = Vec::new();
            while i < bytes.len() && bytes[i] != b')' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                }
                if bytes[i].is_ascii_graphic() || bytes[i] == b' ' {
                    buf.push(bytes[i]);
                }
                i += 1;
            }
            if buf.len() >= 3 {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&String::from_utf8_lossy(&buf));
            }
        }
        i += 1;
    }
    out
}
