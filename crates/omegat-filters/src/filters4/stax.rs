//! StAX-like events and a writer that follows JDK `XMLStreamWriter`
//! (self-closing empty elements, double-quoted attributes).

use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QName {
    pub prefix: String,
    pub local: String,
    pub uri: String,
}

impl QName {
    pub fn new(prefix: impl Into<String>, local: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            local: local.into(),
            uri: uri.into(),
        }
    }

    pub fn local(local: impl Into<String>) -> Self {
        Self::new("", local, "")
    }

    pub fn written_name(&self) -> String {
        if self.prefix.is_empty() {
            self.local.clone()
        } else {
            format!("{}:{}", self.prefix, self.local)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: QName,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlEvent {
    StartDocument {
        version: Option<String>,
        encoding: Option<String>,
        standalone: Option<String>,
    },
    StartElement {
        name: QName,
        attrs: Vec<Attribute>,
        namespaces: Vec<(String, String)>,
    },
    /// Java StAX `XMLEvent.ATTRIBUTE` (OpenXmlFilter rewrites `w:lang` this way).
    Attribute {
        name: QName,
        value: String,
    },
    EndElement {
        name: QName,
    },
    Characters {
        data: String,
    },
    CData {
        data: String,
    },
    Comment {
        text: String,
    },
    Pi {
        target: String,
        data: String,
    },
    EntityRef {
        name: String,
    },
    DocType {
        text: String,
    },
    EndDocument,
}

impl XmlEvent {
    pub fn as_start(&self) -> Option<(&QName, &[Attribute], &[(String, String)])> {
        match self {
            XmlEvent::StartElement {
                name,
                attrs,
                namespaces,
            } => Some((name, attrs, namespaces)),
            _ => None,
        }
    }

    pub fn local_name(&self) -> Option<&str> {
        match self {
            XmlEvent::StartElement { name, .. } | XmlEvent::EndElement { name } => Some(&name.local),
            _ => None,
        }
    }

    pub fn attr(&self, local: &str) -> Option<&str> {
        match self {
            XmlEvent::StartElement { attrs, .. } => attrs
                .iter()
                .find(|a| a.name.local == local)
                .map(|a| a.value.as_str()),
            _ => None,
        }
    }
}

#[derive(Default)]
struct NsScope {
    /// prefix → uri; empty prefix is the default namespace
    map: Vec<(String, String)>,
}

fn resolve_uri(stack: &[NsScope], prefix: &str) -> String {
    for scope in stack.iter().rev() {
        if let Some((_, uri)) = scope.map.iter().rev().find(|(p, _)| p == prefix) {
            return uri.clone();
        }
    }
    if prefix == "xml" {
        return "http://www.w3.org/XML/1998/namespace".into();
    }
    if prefix == "xmlns" {
        return "http://www.w3.org/2000/xmlns/".into();
    }
    String::new()
}

fn split_name(raw: &str) -> (String, String) {
    if let Some((p, l)) = raw.split_once(':') {
        (p.to_string(), l.to_string())
    } else {
        (String::new(), raw.to_string())
    }
}

pub fn read_xml_events(raw: &str) -> Result<Vec<XmlEvent>, String> {
    let mut reader = Reader::from_str(raw);
    let cfg = reader.config_mut();
    cfg.trim_text(false);
    cfg.expand_empty_elements = true;
    cfg.check_end_names = false;

    let mut events = Vec::new();
    let mut buf = Vec::new();
    let mut ns_stack: Vec<NsScope> = vec![NsScope::default()];

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(d)) => {
                let version = d
                    .version()
                    .ok()
                    .and_then(|v| String::from_utf8(v.as_ref().to_vec()).ok());
                let encoding = d
                    .encoding()
                    .and_then(|r| r.ok())
                    .and_then(|v| String::from_utf8(v.as_ref().to_vec()).ok());
                events.push(XmlEvent::StartDocument {
                    version,
                    encoding,
                    standalone: None,
                });
            }
            Ok(Event::Start(e)) => {
                let elem_raw = reader
                    .decoder()
                    .decode(e.name().as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                let mut namespaces = Vec::new();
                let mut attrs = Vec::new();
                let mut scope = NsScope::default();
                for a in e.attributes().with_checks(false).flatten() {
                    let key = reader
                        .decoder()
                        .decode(a.key.as_ref())
                        .map_err(|err| err.to_string())?
                        .into_owned();
                    let val = a
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|err| err.to_string())?
                        .into_owned();
                    if key == "xmlns" {
                        scope.map.push((String::new(), val.clone()));
                        namespaces.push((String::new(), val));
                    } else if let Some(p) = key.strip_prefix("xmlns:") {
                        scope.map.push((p.to_string(), val.clone()));
                        namespaces.push((p.to_string(), val));
                    } else {
                        let (prefix, local) = split_name(&key);
                        attrs.push(Attribute {
                            name: QName::new(prefix, local, String::new()),
                            value: val,
                        });
                    }
                }
                ns_stack.push(scope);
                let (prefix, local) = split_name(&elem_raw);
                let uri = resolve_uri(&ns_stack, &prefix);
                for a in &mut attrs {
                    a.name.uri = resolve_uri(&ns_stack, &a.name.prefix);
                }
                events.push(XmlEvent::StartElement {
                    name: QName::new(prefix, local, uri),
                    attrs,
                    namespaces,
                });
            }
            Ok(Event::End(e)) => {
                let elem_raw = reader
                    .decoder()
                    .decode(e.name().as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                let (prefix, local) = split_name(&elem_raw);
                let uri = resolve_uri(&ns_stack, &prefix);
                if ns_stack.len() > 1 {
                    ns_stack.pop();
                }
                events.push(XmlEvent::EndElement {
                    name: QName::new(prefix, local, uri),
                });
            }
            Ok(Event::Text(t)) => {
                let data = t
                    .unescape()
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| {
                        reader
                            .decoder()
                            .decode(t.as_ref())
                            .map(|c| c.into_owned())
                            .unwrap_or_default()
                    });
                // XML 1.0 §2.11: parsers normalize CR LF / CR to LF.
                events.push(XmlEvent::Characters {
                    data: normalize_xml_newlines(&data),
                });
            }
            Ok(Event::CData(t)) => {
                let data = reader
                    .decoder()
                    .decode(t.as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                events.push(XmlEvent::CData {
                    data: normalize_xml_newlines(&data),
                });
            }
            Ok(Event::Comment(t)) => {
                let text = reader
                    .decoder()
                    .decode(t.as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                events.push(XmlEvent::Comment { text });
            }
            Ok(Event::PI(p)) => {
                let raw = reader
                    .decoder()
                    .decode(p.as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                let (target, data) = raw
                    .split_once(|c: char| c.is_whitespace())
                    .map(|(t, d)| (t.to_string(), d.to_string()))
                    .unwrap_or((raw, String::new()));
                events.push(XmlEvent::Pi { target, data });
            }
            Ok(Event::DocType(d)) => {
                let text = reader
                    .decoder()
                    .decode(d.as_ref())
                    .map_err(|err| err.to_string())?
                    .into_owned();
                events.push(XmlEvent::DocType { text });
            }
            Ok(Event::Eof) => {
                events.push(XmlEvent::EndDocument);
                break;
            }
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
        buf.clear();
    }
    Ok(events)
}

pub struct StaxWriter {
    pub out: String,
    pending_start: bool,
    stack: Vec<QName>,
}

impl Default for StaxWriter {
    fn default() -> Self {
        Self {
            out: String::new(),
            pending_start: false,
            stack: Vec::new(),
        }
    }
}

impl StaxWriter {
    pub fn close_pending_start(&mut self) {
        if self.pending_start {
            self.out.push('>');
            self.pending_start = false;
        }
    }

    pub fn write_start_element(&mut self, prefix: &str, local: &str, uri: &str) {
        self.close_pending_start();
        self.out.push('<');
        if !prefix.is_empty() {
            self.out.push_str(prefix);
            self.out.push(':');
        }
        self.out.push_str(local);
        self.pending_start = true;
        self.stack.push(QName::new(prefix, local, uri));
    }

    pub fn write_namespace(&mut self, prefix: &str, uri: &str) {
        if prefix.is_empty() {
            self.out.push_str(" xmlns=\"");
        } else {
            self.out.push_str(" xmlns:");
            self.out.push_str(prefix);
            self.out.push_str("=\"");
        }
        self.out.push_str(&escape_attr(uri));
        self.out.push('"');
    }

    pub fn write_attribute(&mut self, prefix: &str, _uri: &str, local: &str, value: &str) {
        self.out.push(' ');
        if !prefix.is_empty() {
            self.out.push_str(prefix);
            self.out.push(':');
        }
        self.out.push_str(local);
        self.out.push_str("=\"");
        self.out.push_str(&escape_attr(value));
        self.out.push('"');
    }

    pub fn write_end_element(&mut self) {
        let name = self.stack.pop().unwrap_or_else(|| QName::local(""));
        if self.pending_start {
            self.out.push_str("/>");
            self.pending_start = false;
            return;
        }
        self.out.push_str("</");
        if !name.prefix.is_empty() {
            self.out.push_str(&name.prefix);
            self.out.push(':');
        }
        self.out.push_str(&name.local);
        self.out.push('>');
    }

    /// Java `XMLStreamWriter.close()` / `writeEndDocument`: close leftover
    /// open elements (Office LOOP2 can leave the writer stack unbalanced).
    pub fn close_remaining(&mut self) {
        while !self.stack.is_empty() {
            self.write_end_element();
        }
    }

    pub fn write_characters(&mut self, data: &str) {
        self.close_pending_start();
        self.out.push_str(&escape_text(data));
    }

    pub fn write_cdata(&mut self, data: &str) {
        self.close_pending_start();
        self.out.push_str("<![CDATA[");
        self.out.push_str(data);
        self.out.push_str("]]>");
    }

    pub fn write_comment(&mut self, text: &str) {
        self.close_pending_start();
        self.out.push_str("<!--");
        self.out.push_str(text);
        self.out.push_str("-->");
    }

    pub fn write_pi(&mut self, target: &str, data: &str) {
        self.close_pending_start();
        self.out.push_str("<?");
        self.out.push_str(target);
        if !data.is_empty() {
            self.out.push(' ');
            self.out.push_str(data);
        }
        self.out.push_str("?>");
    }

    pub fn write_entity_ref(&mut self, name: &str) {
        self.close_pending_start();
        self.out.push('&');
        self.out.push_str(name);
        self.out.push(';');
    }

    pub fn write_dtd(&mut self, text: &str) {
        self.close_pending_start();
        self.out.push_str("<!DOCTYPE ");
        self.out.push_str(text);
        self.out.push('>');
    }
}

pub fn from_event_to_writer(ev: &XmlEvent, writer: &mut StaxWriter) {
    match ev {
        XmlEvent::StartDocument { .. } => {}
        XmlEvent::StartElement {
            name,
            attrs,
            namespaces,
        } => {
            writer.write_start_element(&name.prefix, &name.local, &name.uri);
            for (p, uri) in namespaces {
                writer.write_namespace(p, uri);
            }
            for a in attrs {
                writer.write_attribute(&a.name.prefix, &a.name.uri, &a.name.local, &a.value);
            }
        }
        XmlEvent::Attribute { name, value } => {
            writer.write_attribute(&name.prefix, &name.uri, &name.local, value);
        }
        XmlEvent::EndElement { .. } => writer.write_end_element(),
        XmlEvent::Characters { data } => writer.write_characters(data),
        XmlEvent::CData { data } => writer.write_cdata(data),
        XmlEvent::Comment { text } => writer.write_comment(text),
        XmlEvent::Pi { target, data } => writer.write_pi(target, data),
        XmlEvent::EntityRef { name } => writer.write_entity_ref(name),
        XmlEvent::DocType { text } => writer.write_dtd(text),
        XmlEvent::EndDocument => {}
    }
}

pub fn escape_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

pub fn escape_attr(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Java `XMLStreamReader.standaloneSet` / `isStandalone`.
pub fn detect_xml_standalone(raw: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"standalone\s*=\s*"(yes|no)""#).unwrap());
    re.captures(raw).map(|c| c[1].to_string())
}

/// Java `PatternConsts.XML_ENCODING` — double quotes only.
pub fn detect_xml_encoding(raw: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"<\?xml.*?encoding\s*=\s*"(\S+?)".*?\?>"#).unwrap()
    });
    re.captures(raw).map(|c| c[1].to_string())
}

/// Java `XMLReader.detectEndOfLine`.
pub fn detect_eol(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                return "\r\n".into();
            }
            return "\r".into();
        }
        if bytes[i] == b'\n' {
            return "\n".into();
        }
        i += 1;
    }
    "\n".into()
}

/// XML 1.0 line-ending normalization (CRLF / CR → LF).
pub fn normalize_xml_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// Java `XMLWriter`: replace first `<?xml ...?>` then map `\n` → detected EOL.
/// Assumes the body uses LF only (StAX / XML 1.0). Existing CRLF is not
/// expanded a second time.
/// How Java emits the XML prolog for filters4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmlDeclStyle {
    /// `AbstractXmlFilter` START_DOCUMENT: `<?xml  version="…" encoding="…" ?>`
    /// (space after `xml`, space before `?>`). Used for ZIP inner parts
    /// (`AbstractZipFilter` + `XMLReader`).
    AbstractXml,
    /// Woodstox `XMLStreamWriter.writeStartDocument`: `<?xml version="1.0"?>`
    /// or `<?xml version="1.0" encoding="UTF-8"?>`. Used for standalone
    /// XLIFF / SDL XLIFF files (Reader already decoded; encoding often omitted).
    Woodstox,
}

/// Java `AbstractXmlFilter` START_DOCUMENT / Woodstox `writeStartDocument`.
pub fn java_xml_declaration(
    version: Option<&str>,
    encoding: Option<&str>,
    standalone: Option<&str>,
    style: XmlDeclStyle,
) -> String {
    match style {
        XmlDeclStyle::AbstractXml => {
            let mut sb = String::from("<?xml ");
            if let Some(v) = version.filter(|s| !s.is_empty()) {
                sb.push_str(" version=\"");
                sb.push_str(v);
                sb.push('"');
            }
            if let Some(e) = encoding.filter(|s| !s.is_empty()) {
                sb.push_str(" encoding=\"");
                sb.push_str(e);
                sb.push('"');
            }
            if let Some(s) = standalone.filter(|s| !s.is_empty()) {
                sb.push_str(" standalone=\"");
                sb.push_str(s);
                sb.push('"');
            }
            sb.push_str(" ?>");
            sb
        }
        XmlDeclStyle::Woodstox => {
            let mut sb = String::from("<?xml");
            if let Some(v) = version.filter(|s| !s.is_empty()) {
                sb.push_str(" version=\"");
                sb.push_str(v);
                sb.push('"');
            }
            if let Some(e) = encoding.filter(|s| !s.is_empty()) {
                sb.push_str(" encoding=\"");
                sb.push_str(e);
                sb.push('"');
            }
            if let Some(s) = standalone.filter(|s| !s.is_empty()) {
                sb.push_str(" standalone=\"");
                sb.push_str(s);
                sb.push('"');
            }
            sb.push_str("?>");
            sb
        }
    }
}

pub fn finalize_xml_writer(body: &str, encoding: Option<&str>, eol: &str) -> String {
    finalize_xml_writer_ex(body, encoding, None, eol, XmlDeclStyle::AbstractXml)
}

pub fn finalize_xml_writer_ex(
    body: &str,
    encoding: Option<&str>,
    standalone: Option<&str>,
    eol: &str,
    style: XmlDeclStyle,
) -> String {
    let header = java_xml_declaration(Some("1.0"), encoding, standalone, style);
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"<\?xml.*?\?>").unwrap());
    let with_header = if re.is_match(body) {
        re.replace(body, header.as_str()).into_owned()
    } else {
        format!("{header}{body}")
    };
    if eol == "\n" {
        return with_header;
    }
    let mut out = String::with_capacity(with_header.len() + 8);
    let chars: Vec<char> = with_header.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\r' && i + 1 < chars.len() && chars[i + 1] == '\n' {
            out.push_str(eol);
            i += 2;
        } else if chars[i] == '\n' {
            out.push_str(eol);
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub fn characters_of(events: &[XmlEvent]) -> String {
    let mut s = String::new();
    for ev in events {
        match ev {
            XmlEvent::Characters { data } | XmlEvent::CData { data } => s.push_str(data),
            _ => {}
        }
    }
    s
}
