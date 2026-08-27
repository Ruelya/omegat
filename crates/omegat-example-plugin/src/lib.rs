//! Example cdylib: registers a `*.example` filter and executable Marker.

use serde_json::{json, Value};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
pub struct OmegatPluginHost {
    pub ctx: *mut c_void,
    pub register_filter: Option<
        extern "C" fn(
            ctx: *mut c_void,
            id: *const c_char,
            name: *const c_char,
            masks: *const c_char,
            parse: extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int,
            write: extern "C" fn(*const c_char, *const c_char, *const c_char) -> c_int,
        ),
    >,
    pub register_mt:
        Option<extern "C" fn(ctx: *mut c_void, id: *const c_char, name: *const c_char)>,
    pub register_tokenizer:
        Option<extern "C" fn(ctx: *mut c_void, id: *const c_char, name: *const c_char)>,
    pub register_marker: Option<
        extern "C" fn(
            ctx: *mut c_void,
            id: *const c_char,
            name: *const c_char,
            mark: extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int,
        ),
    >,
}

pub fn parse_example_text(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| (i.to_string(), line.to_string()))
        .collect()
}

pub fn write_example_text(source: &str, translations: &[(String, String)]) -> String {
    let map: std::collections::HashMap<&str, &str> = translations
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out = String::new();
    let mut i = 0usize;
    for line in source.lines() {
        if line.trim().is_empty() {
            out.push('\n');
            continue;
        }
        let key = i.to_string();
        out.push_str(map.get(key.as_str()).copied().unwrap_or(line));
        out.push('\n');
        i += 1;
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn segments_json(pairs: &[(String, String)]) -> String {
    let mut out = String::from("{\"segments\":[");
    for (i, (id, source)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"id\":\"");
        out.push_str(&json_escape(id));
        out.push_str("\",\"source\":\"");
        out.push_str(&json_escape(source));
        out.push_str("\"}");
    }
    out.push_str("]}");
    out
}

pub fn example_marker_output(input: &Value) -> Value {
    let text = input
        .get("translation_text")
        .and_then(Value::as_str)
        .unwrap_or("");
    let file = input
        .get("entry_key")
        .and_then(|key| key.get("file"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let marks = text
        .match_indices("plugin")
        .map(|(byte_start, needle)| {
            let start_offset = text[..byte_start].encode_utf16().count();
            json!({
                "start_offset": start_offset,
                "end_offset": start_offset + needle.encode_utf16().count(),
                "painter": "native-plugin",
                "painter_color": "#7c3aed",
                "tooltip_text": format!("Example marker in {file}"),
                "entry_part": "TRANSLATION"
            })
        })
        .collect::<Vec<_>>();
    json!({ "marks": marks })
}

/// Minimal `{"0":"a","1":"b"}` object reader (no nested objects).
pub fn parse_translation_object(json: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = json.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            if bytes[i] == b'\\' {
                i += 1;
            }
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let key = String::from_utf8_lossy(&bytes[start..i]).into_owned();
        i += 1;
        while i < bytes.len() && bytes[i] != b'"' {
            if bytes[i] == b'}' {
                return out;
            }
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'"' {
            break;
        }
        i += 1;
        let mut val = String::new();
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'\\' && i + 1 < bytes.len() {
                val.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if c == b'"' {
                break;
            }
            val.push(c as char);
            i += 1;
        }
        out.push((key, val));
        i += 1;
    }
    out
}

fn write_cstr(out: *mut c_char, cap: c_int, s: &str) -> c_int {
    if out.is_null() || cap <= 0 {
        return -1;
    }
    let Ok(c) = CString::new(s) else {
        return -1;
    };
    let bytes = c.as_bytes_with_nul();
    if bytes.len() > cap as usize {
        return -1;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, bytes.len());
    }
    (bytes.len() - 1) as c_int
}

#[no_mangle]
pub extern "C" fn omegat_plugin_abi() -> *const c_char {
    b"{\"id\":\"example\",\"name\":\"Example Filter\",\"version\":\"1.0.0\",\"kind\":\"filter\"}\0"
        .as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn omegat_plugin_register(host: *const OmegatPluginHost) {
    if host.is_null() {
        return;
    }
    let host = unsafe { &*host };
    if let Some(reg) = host.register_filter {
        reg(
            host.ctx,
            b"example\0".as_ptr() as *const c_char,
            b"Example Filter\0".as_ptr() as *const c_char,
            b"*.example\0".as_ptr() as *const c_char,
            omegat_plugin_filter_parse,
            omegat_plugin_filter_write,
        );
    }
    if let Some(reg) = host.register_marker {
        reg(
            host.ctx,
            b"example.native-marker\0".as_ptr() as *const c_char,
            b"org.omegat.example.NativePluginMarker\0".as_ptr() as *const c_char,
            omegat_plugin_marker_marks,
        );
    }
}

#[no_mangle]
pub extern "C" fn omegat_plugin_marker_marks(
    input_json: *const c_char,
    out: *mut c_char,
    cap: c_int,
) -> c_int {
    if input_json.is_null() {
        return -1;
    }
    let input = unsafe { CStr::from_ptr(input_json) };
    let Ok(input) = serde_json::from_slice::<Value>(input.to_bytes()) else {
        return -1;
    };
    if input
        .get("crash_worker")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        std::process::abort();
    }
    let output = example_marker_output(&input).to_string();
    write_cstr(out, cap, &output)
}

#[no_mangle]
pub extern "C" fn omegat_plugin_filter_parse(
    path: *const c_char,
    out: *mut c_char,
    cap: c_int,
) -> c_int {
    if path.is_null() {
        return -1;
    }
    let path = unsafe { CStr::from_ptr(path) }.to_string_lossy();
    let Ok(text) = std::fs::read_to_string(path.as_ref()) else {
        return -1;
    };
    let json = segments_json(&parse_example_text(&text));
    write_cstr(out, cap, &json)
}

#[no_mangle]
pub extern "C" fn omegat_plugin_filter_write(
    src: *const c_char,
    dest: *const c_char,
    translations_json: *const c_char,
) -> c_int {
    if src.is_null() || dest.is_null() {
        return -1;
    }
    let src = unsafe { CStr::from_ptr(src) }.to_string_lossy();
    let dest = unsafe { CStr::from_ptr(dest) }.to_string_lossy();
    let json = if translations_json.is_null() {
        "{}"
    } else {
        unsafe { CStr::from_ptr(translations_json) }
            .to_str()
            .unwrap_or("{}")
    };
    let Ok(text) = std::fs::read_to_string(src.as_ref()) else {
        return -1;
    };
    let tr = parse_translation_object(json);
    let out = write_example_text(&text, &tr);
    if let Some(parent) = std::path::Path::new(dest.as_ref()).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(dest.as_ref(), out) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_write_roundtrip() {
        let src = "Hello from plugin\n\nSecond line\n";
        let segs = parse_example_text(src);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].1, "Hello from plugin");
        let written = write_example_text(
            src,
            &[("0".into(), "Bonjour".into()), ("1".into(), "Deux".into())],
        );
        assert!(written.contains("Bonjour"));
        assert!(written.contains("Deux"));
    }

    #[test]
    fn translation_object_reader() {
        let v = parse_translation_object(r#"{"0":"A","1":"B"}"#);
        assert_eq!(v, vec![("0".into(), "A".into()), ("1".into(), "B".into())]);
    }

    #[test]
    fn marker_uses_utf16_offsets_and_complete_entry_key() {
        let output = example_marker_output(&json!({
            "entry_key": {
                "file": "source/example.txt",
                "source_text": "plugin",
                "id": "same",
                "prev": "",
                "next": null,
                "path": "body"
            },
            "source_text": "plugin",
            "translation_text": "😀 plugin and plugin",
            "is_active": true
        }));
        assert_eq!(
            output,
            json!({
                "marks": [
                    {
                        "start_offset": 3,
                        "end_offset": 9,
                        "painter": "native-plugin",
                        "painter_color": "#7c3aed",
                        "tooltip_text": "Example marker in source/example.txt",
                        "entry_part": "TRANSLATION"
                    },
                    {
                        "start_offset": 14,
                        "end_offset": 20,
                        "painter": "native-plugin",
                        "painter_color": "#7c3aed",
                        "tooltip_text": "Example marker in source/example.txt",
                        "entry_part": "TRANSLATION"
                    }
                ]
            })
        );
    }
}
