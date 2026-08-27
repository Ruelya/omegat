//! Java `org.omegat.filters2.html2.HTMLWriter` + `org.omegat.util.HTMLUtils`
//! / `EntityUtil`.

use super::html_options::{HtmlOptions, RewriteMode};
use regex::Regex;
use std::sync::OnceLock;

/// Java `Character.isWhitespace` (excludes NBSP / figure / narrow NBSP).
pub fn java_is_whitespace(c: char) -> bool {
    if matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}') {
        return false;
    }
    c.is_whitespace()
}

/// Java `String.trim()`: code units `<= 0x20`.
pub fn java_trim(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start] <= 0x20 {
        start += 1;
    }
    while end > start && bytes[end - 1] <= 0x20 {
        end -= 1;
    }
    &s[start..end]
}

pub fn entities_to_chars(input: &str) -> String {
    html_escape::decode_html_entities(input).into_owned()
}

pub fn chars_to_entities(input: &str, encoding: Option<&str>, shortcuts: &[String]) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\u{00A0}' => out.push_str("&nbsp;"),
            '&' => out.push_str("&amp;"),
            '>' => {
                if i > 0 && chars[i - 1] == '?' {
                    out.push('>');
                } else {
                    out.push_str("&gt;");
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '?' {
                    out.push('<');
                } else if let Some((shortcut, consumed)) = match_shortcut(&chars[i..], shortcuts) {
                    out.push_str(shortcut);
                    i += consumed;
                    continue;
                } else {
                    out.push_str("&lt;");
                }
            }
            _ => out.push(ch),
        }
        i += 1;
    }
    if let Some(enc) = encoding {
        rewrite_unencodable(&out, enc)
    } else {
        out
    }
}

fn match_shortcut<'a>(rest: &[char], shortcuts: &'a [String]) -> Option<(&'a str, usize)> {
    let rest_s: String = rest.iter().collect();
    if let Some(gt) = rest_s.find('>') {
        let maybe = &rest_s[..=gt];
        if shortcuts.iter().any(|s| s == maybe) {
            return Some((
                shortcuts.iter().find(|s| s.as_str() == maybe).unwrap(),
                maybe.chars().count(),
            ));
        }
    }
    None
}

fn rewrite_unencodable(contents: &str, encoding: &str) -> String {
    let Some(enc) = encoding_rs::Encoding::for_label(encoding.as_bytes()) else {
        return contents.to_string();
    };
    if enc == encoding_rs::UTF_8 || enc == encoding_rs::UTF_16LE || enc == encoding_rs::UTF_16BE {
        return contents.to_string();
    }
    let mut out = String::new();
    for ch in contents.chars() {
        let s = ch.to_string();
        let (_, _, unmappable) = enc.encode(&s);
        if unmappable {
            out.push_str(&format!("&#{};", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn space_prefix(input: &str, compress: bool) -> String {
    let mut n = 0;
    for c in input.chars() {
        if !java_is_whitespace(c) {
            if n == 0 {
                return String::new();
            }
            if compress {
                return input.chars().next().unwrap().to_string();
            }
            return input.chars().take(n).collect();
        }
        n += 1;
    }
    String::new()
}

pub fn space_postfix(input: &str, compress: bool) -> String {
    let chars: Vec<char> = input.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut n = 0;
    for c in chars.iter().rev() {
        if !java_is_whitespace(*c) {
            break;
        }
        n += 1;
    }
    if n == 0 {
        return String::new();
    }
    if n == chars.len() {
        return String::new();
    }
    let skip = chars.len() - n;
    if compress {
        chars[skip].to_string()
    } else {
        chars[skip..].iter().collect()
    }
}

/// Java `StringUtil.compressSpaces`.
pub fn compress_spaces(s: &str) -> String {
    let mut out = String::new();
    let mut was_space = true;
    for ch in s.chars() {
        if java_is_whitespace(ch) {
            if !was_space {
                was_space = true;
            }
        } else {
            if was_space && !out.is_empty() {
                out.push(' ');
            }
            out.push(ch);
            was_space = false;
        }
    }
    out
}

pub fn compress_whitespace_layout(input: &str, enabled: bool) -> String {
    if !enabled {
        return input.to_string();
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"( |\t)+").unwrap());
    re.replace_all(input, " ").into_owned()
}

fn first_eol(contents: &str) -> &str {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\r?\n|\r[^\n]").unwrap());
    re.find(contents).map(|m| m.as_str()).unwrap_or("\n")
}

/// Java `HTMLWriter.flush` charset rewrite. Dot does **not** match newlines
/// (Java `PatternConsts` has no DOTALL).
pub fn rewrite_encoding_header(contents: &str, encoding: &str, options: &HtmlOptions) -> String {
    if options.rewrite_encoding == RewriteMode::Never || encoding.is_empty() {
        return contents.to_string();
    }
    let mut contents = contents.to_string();
    let eol = first_eol(&contents).to_string();

    static XML_HEADER: OnceLock<Regex> = OnceLock::new();
    let xml_re = XML_HEADER.get_or_init(|| Regex::new(r"(<\?xml.*?\?>)").unwrap());
    let mut xhtml = false;
    if xml_re.is_match(&contents) {
        let header = format!("<?xml version=\"1.0\" encoding=\"{encoding}\"?>");
        contents = xml_re.replace(&contents, header.as_str()).into_owned();
        xhtml = true;
    }

    let html_meta = if xhtml {
        format!(
            "<meta http-equiv=\"content-type\" content=\"text/html; charset={encoding}\" />"
        )
    } else {
        format!("<meta http-equiv=\"content-type\" content=\"text/html; charset={encoding}\">")
    };

    static HTML_ENC: OnceLock<Regex> = OnceLock::new();
    let enc_re = HTML_ENC.get_or_init(|| {
        Regex::new(
            r#"(?i)<meta.*?content\s*=\s*["']\s*text/html\s*;\s*charset\s*=\s*(\S+?)["'].*?/?\s*>"#,
        )
        .unwrap()
    });
    static HTML5_ENC: OnceLock<Regex> = OnceLock::new();
    let enc5_re = HTML5_ENC.get_or_init(|| {
        Regex::new(r#"(?i)<meta.*?charset\s*=\s*["'](\S+?)["'].*?/?\s*>"#).unwrap()
    });
    static HTML_HEAD: OnceLock<Regex> = OnceLock::new();
    let head_re = HTML_HEAD.get_or_init(|| Regex::new(r"(?i)<head[^e]*?>").unwrap());
    static HTML_HTML: OnceLock<Regex> = OnceLock::new();
    let html_re = HTML_HTML.get_or_init(|| Regex::new(r"(?i)<html.*?>").unwrap());

    if enc_re.is_match(&contents) {
        contents = enc_re.replace(&contents, html_meta.as_str()).into_owned();
    } else if enc5_re.is_match(&contents) {
        contents = enc5_re
            .replace(&contents, format!("<meta charset=\"{encoding}\">").as_str())
            .into_owned();
    } else if options.rewrite_encoding != RewriteMode::IfMeta {
        if head_re.is_match(&contents) {
            let repl = format!("$0{eol}    {html_meta}");
            contents = head_re.replace(&contents, repl.as_str()).into_owned();
        } else if options.rewrite_encoding != RewriteMode::IfHeader {
            if html_re.is_match(&contents) {
                let repl = format!("$0{eol}<head>{eol}    {html_meta}{eol}</head>");
                contents = html_re.replace(&contents, repl.as_str()).into_owned();
            } else {
                contents = format!(
                    "<html>{eol}<head>{eol}    {html_meta}{eol}</head>{eol}{contents}"
                );
            }
        }
    }
    contents
}
