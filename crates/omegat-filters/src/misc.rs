//! P3 leftover text-like formats.

use crate::{
    apply_skeleton, ensure_parent, placeholder, read_to_string, ExtractedSegment, Filter,
    FilterContext, ParsedFile, Result,
};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

macro_rules! simple_filter {
    ($ty:ident, $id:expr, $name:expr, $masks:expr, $phase:expr, $parser:ident) => {
        pub struct $ty;
        impl Filter for $ty {
            fn id(&self) -> &'static str {
                $id
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn default_masks(&self) -> &'static [&'static str] {
                $masks
            }
            fn phase(&self) -> u8 {
                $phase
            }
            fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
                $parser(&read_to_string(path)?)
            }
            fn write(
                &self,
                source_path: &Path,
                dest_path: &Path,
                translations: &HashMap<String, String>,
                _ctx: &FilterContext,
            ) -> Result<()> {
                let parsed = $parser(&read_to_string(source_path)?)?;
                let out = parsed
                    .skeleton
                    .map(|sk| apply_skeleton(&sk, translations))
                    .unwrap_or_else(|| read_to_string(source_path).unwrap_or_default());
                ensure_parent(dest_path)?;
                std::fs::write(dest_path, out)?;
                Ok(())
            }
        }
    };
}

fn kv_parser(raw: &str, assign: &[char]) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            skeleton.push_str(line);
            skeleton.push('\n');
            continue;
        }
        if let Some(pos) = trimmed.find(assign) {
            let (k, v) = trimmed.split_at(pos);
            let v = v.trim_start_matches(assign).trim();
            if v.is_empty() {
                skeleton.push_str(line);
                skeleton.push('\n');
                continue;
            }
            skeleton.push_str(k);
            if trimmed[pos..].starts_with('=') {
                skeleton.push('=');
            } else {
                skeleton.push_str(&trimmed[pos..pos + 1]);
            }
            skeleton.push_str(&placeholder(segments.len()));
            skeleton.push('\n');
            segments.push(ExtractedSegment {
                id: segments.len().to_string(),
                source: v.trim_matches('"').to_string(),
                existing_translation: None,
                note: None,
                comment: None,
                path: Some(k.trim().to_string()),
                protected_parts: vec![],
            });
        } else {
            skeleton.push_str(line);
            skeleton.push('\n');
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn parse_ini(raw: &str) -> Result<ParsedFile> {
    kv_parser(raw, &['='])
}
fn parse_rc(raw: &str) -> Result<ParsedFile> {
    let re = Regex::new(r#"(CAPTION|LTEXT|PUSHBUTTON|CONTROL|MENUITEM)\s+"([^"]+)""#).unwrap();
    token_replace(raw, &re, 2)
}
fn parse_latex(raw: &str) -> Result<ParsedFile> {
    let re = Regex::new(r"\\(caption|section|subsection|title|author|chapter)\{([^}]*)\}").unwrap();
    token_replace(raw, &re, 2)
}
fn parse_dtd(raw: &str) -> Result<ParsedFile> {
    let re = Regex::new(r#"<!ENTITY\s+(\S+)\s+"([^"]*)">"#).unwrap();
    token_replace(raw, &re, 2)
}
fn parse_lang(raw: &str) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for line in raw.lines() {
        if line.starts_with(';') || line.trim().is_empty() {
            skeleton.push_str(line);
            skeleton.push('\n');
        } else {
            skeleton.push_str(&placeholder(segments.len()));
            skeleton.push('\n');
            segments.push(ExtractedSegment {
                id: segments.len().to_string(),
                source: line.to_string(),
                existing_translation: None,
                note: None,
                comment: None,
                path: None,
                protected_parts: vec![],
            });
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}
fn parse_ftl(raw: &str) -> Result<ParsedFile> {
    kv_parser(raw, &['='])
}
fn parse_php(raw: &str) -> Result<ParsedFile> {
    let re = Regex::new(r#"\$string\[[^\]]+\]\s*=\s*'([^']*)'"#).unwrap();
    token_replace(raw, &re, 1)
}
fn parse_wiki(raw: &str) -> Result<ParsedFile> {
    crate_text_paragraphs(raw)
}
fn parse_ilias(raw: &str) -> Result<ParsedFile> {
    kv_parser(raw, &['#'])
}
fn parse_xtag(raw: &str) -> Result<ParsedFile> {
    crate_text_paragraphs(raw)
}
fn parse_hhc(raw: &str) -> Result<ParsedFile> {
    let re = Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
    token_replace(raw, &re, 1)
}
fn parse_magento(raw: &str) -> Result<ParsedFile> {
    kv_parser(raw, &[','])
}

fn crate_text_paragraphs(raw: &str) -> Result<ParsedFile> {
    let normalized = raw.replace("\r\n", "\n");
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for (i, part) in normalized.split("\n\n").enumerate() {
        if i > 0 {
            skeleton.push_str("\n\n");
        }
        if part.trim().is_empty() {
            skeleton.push_str(part);
            continue;
        }
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: part.to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: None,
            protected_parts: vec![],
        });
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn token_replace(raw: &str, re: &Regex, group: usize) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut last = 0usize;
    for cap in re.captures_iter(raw) {
        let m = cap.get(group).unwrap();
        skeleton.push_str(&raw[last..m.start()]);
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: m.as_str().to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: cap.get(1).map(|g| g.as_str().to_string()),
            protected_parts: vec![],
        });
        last = m.end();
    }
    skeleton.push_str(&raw[last..]);
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

simple_filter!(LatexFilter, "latex", "LaTeX", &["*.tex"], 3, parse_latex);
simple_filter!(RcFilter, "rc", "Windows Resources", &["*.rc"], 3, parse_rc);
simple_filter!(MoodlePhpFilter, "moodlephp", "Moodle PHP", &["*.php"], 3, parse_php);
simple_filter!(MozillaDtdFilter, "mozdtd", "Mozilla DTD", &["*.dtd"], 3, parse_dtd);
simple_filter!(MozillaLangFilter, "mozlang", "Mozilla Lang", &["*.lang"], 3, parse_lang);
simple_filter!(MozillaFtlFilter, "mozftl", "Mozilla FTL", &["*.ftl"], 3, parse_ftl);
simple_filter!(HhcFilter, "hhc", "HTML Help Compiler", &["*.hhc"], 3, parse_hhc);
simple_filter!(IniFilter, "ini", "Key=Value Text", &["*.ini"], 3, parse_ini);
simple_filter!(DokuWikiFilter, "dokuwiki", "DokuWiki", &["*.dokuwiki"], 3, parse_wiki);
simple_filter!(MagentoFilter, "magento", "Magento CE Locale CSV", &["*.csv"], 3, parse_magento);
simple_filter!(IliasFilter, "ilias", "ILIAS Language File", &["*.lang"], 3, parse_ilias);
simple_filter!(XtagFilter, "xtag", "QuarkXPress CopyFlow Gold", &["*.xtg"], 3, parse_xtag);
