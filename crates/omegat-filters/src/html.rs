use crate::{
    apply_skeleton, ensure_parent, extract_tags, placeholder, read_to_string, ExtractedSegment,
    Filter, FilterContext, ParsedFile, ProtectedPart, Result,
};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct HtmlFilter;

impl Filter for HtmlFilter {
    fn id(&self) -> &'static str {
        "html"
    }
    fn name(&self) -> &'static str {
        "HTML and XHTML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.html", "*.htm"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_html(&read_to_string(path)?)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let parsed = parse_html(&read_to_string(source_path)?)?;
        let out = parsed
            .skeleton
            .map(|sk| apply_skeleton(&sk, translations))
            .unwrap_or_default();
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_html(raw: &str) -> Result<ParsedFile> {
    let skip = Regex::new(
        r"(?is)<script\b[^>]*>.*?</script>|<style\b[^>]*>.*?</style>|<noscript\b[^>]*>.*?</noscript>",
    )
    .unwrap();
    let block = Regex::new(
        r"(?is)</?(p|div|h[1-6]|li|td|th|title|label|option|blockquote|pre|dt|dd|figcaption)(\s[^>]*)?>",
    )
    .unwrap();

    let mut work = raw.to_string();
    let mut protected = Vec::new();
    for (i, m) in skip.find_iter(raw).enumerate() {
        let token = format!("<!--OMT_SKIP_{i}-->");
        protected.push((token.clone(), m.as_str().to_string()));
        work = work.replacen(m.as_str(), &token, 1);
    }

    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut last = 0usize;
    for m in block.find_iter(&work) {
        push_text(&work[last..m.start()], &mut segments, &mut skeleton);
        skeleton.push_str(m.as_str());
        last = m.end();
    }
    push_text(&work[last..], &mut segments, &mut skeleton);

    for (token, orig) in protected {
        skeleton = skeleton.replace(&token, &orig);
    }

    if segments.is_empty() {
        let tag = Regex::new(r"(?is)<[^>]+>").unwrap();
        let stripped = tag.replace_all(raw, "\n").to_string();
        let mut segs = Vec::new();
        let mut sk = String::new();
        for chunk in stripped.split('\n') {
            let t = chunk.trim();
            if t.is_empty() {
                sk.push('\n');
                continue;
            }
            sk.push_str(&placeholder(segs.len()));
            sk.push('\n');
            segs.push(text_seg(segs.len(), t));
        }
        return Ok(ParsedFile {
            segments: segs,
            skeleton: Some(sk),
        });
    }

    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn push_text(chunk: &str, segments: &mut Vec<ExtractedSegment>, skeleton: &mut String) {
    let text = html_escape::decode_html_entities(chunk).into_owned();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        skeleton.push_str(chunk);
        return;
    }
    let start = chunk.len() - chunk.trim_start().len();
    let end = chunk.trim_end().len();
    skeleton.push_str(&chunk[..start]);
    skeleton.push_str(&placeholder(segments.len()));
    skeleton.push_str(&chunk[end..]);
    segments.push(text_seg(segments.len(), trimmed));
}

fn text_seg(i: usize, source: &str) -> ExtractedSegment {
    let tags = extract_tags(source);
    ExtractedSegment {
        id: i.to_string(),
        source: source.to_string(),
        existing_translation: None,
        note: None,
        comment: None,
        path: None,
        protected_parts: tags
            .into_iter()
            .map(|t| ProtectedPart {
                text: t,
                details: "tag".into(),
            })
            .collect(),
    }
}
