//! Java `org.omegat.util.HttpConnectionUtils` URL encode/decode.

use once_cell::sync::Lazy;
use regex::Regex;

static HTTP_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bhttps?://\S+").unwrap());

/// Apache `URLCodec` safe set: alphanum + `-_.*` ; space → `+`.
fn codec_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        let c = *b;
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'*') {
            out.push(c as char);
        } else if c == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

fn codec_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for segment in path.split('/') {
        if !segment.is_empty() {
            encoded.push('/');
            encoded.push_str(&codec_encode(segment));
        }
    }
    encoded
}

fn encode_query(query: &str) -> String {
    let mut encoded = String::new();
    for (i, segment) in query.split('&').enumerate() {
        if segment.is_empty() {
            continue;
        }
        if i != 0 {
            encoded.push('&');
        }
        let mut parts = segment.splitn(2, '=');
        let k = parts.next().unwrap_or("");
        let v = parts.next().unwrap_or("");
        encoded.push_str(&codec_encode(k));
        encoded.push('=');
        encoded.push_str(&codec_encode(v));
    }
    encoded
}

fn looks_already_encoded(url: &str) -> bool {
    // Java `UrlValidator.isValid` — already-ASCII wiki paths skip re-encode.
    url.bytes().all(|b| b.is_ascii()) && !url.contains('(')
}

/// Java `HttpConnectionUtils.encodeHttpURLs`.
pub fn encode_http_urls(text: &str) -> String {
    let mut result = String::new();
    let mut last = 0;
    for m in HTTP_URL.find_iter(text) {
        let url = m.as_str();
        result.push_str(&text[last..m.start()]);
        if looks_already_encoded(url) {
            last = m.end();
            result.push_str(url);
            continue;
        }
        if let Some(rest) = url.split_once("://") {
            let scheme = rest.0;
            let after = rest.1;
            let (auth_path, query) = after.split_once('?').unwrap_or((after, ""));
            let (auth, path) = auth_path
                .split_once('/')
                .map(|(a, p)| (a, format!("/{p}")))
                .unwrap_or((auth_path, String::new()));
            result.push_str(scheme);
            result.push_str("://");
            result.push_str(auth);
            if !path.is_empty() {
                result.push_str(&encode_path(&path));
            }
            if !query.is_empty() {
                result.push('?');
                result.push_str(&encode_query(query));
            }
        } else {
            result.push_str(url);
        }
        last = m.end();
    }
    result.push_str(&text[last..]);
    result
}

/// Java `HttpConnectionUtils.decodeHttpURLs`.
pub fn decode_http_urls(text: &str) -> String {
    let mut result = String::new();
    let mut last = 0;
    for m in HTTP_URL.find_iter(text) {
        let url = m.as_str();
        result.push_str(&text[last..m.start()]);
        result.push_str(&codec_decode(url));
        last = m.end();
    }
    result.push_str(&text[last..]);
    result
}
