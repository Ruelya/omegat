//! Source-preserving HTML tokenizer for FilterVisitor.
//!
//! Tokens keep the original inner text of tags (`htmlparser` `Tag.getText()`)
//! so identity write-back can reconstruct `"<" + getText() + ">"`.

#[derive(Debug, Clone)]
pub struct Attr {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Text {
        raw: String,
    },
    Remark {
        raw: String,
    },
    Tag {
        name: String,
        raw_inner: String,
        attrs: Vec<Attr>,
        end_tag: bool,
        empty_xml: bool,
        /// Full original element for protected composite tags (script/style/…).
        protected_html: Option<String>,
    },
}

impl Node {
    pub fn tag_name(&self) -> &str {
        match self {
            Node::Tag { name, .. } => name,
            _ => "",
        }
    }

    pub fn is_end_tag(&self) -> bool {
        matches!(
            self,
            Node::Tag {
                end_tag: true,
                ..
            }
        )
    }

    pub fn is_empty_xml(&self) -> bool {
        matches!(
            self,
            Node::Tag {
                empty_xml: true,
                ..
            }
        )
    }

    pub fn get_text(&self) -> &str {
        match self {
            Node::Text { raw } | Node::Remark { raw } => raw,
            Node::Tag { raw_inner, .. } => raw_inner,
        }
    }

    pub fn to_html(&self) -> String {
        match self {
            Node::Text { raw } => raw.clone(),
            Node::Remark { raw } => raw.clone(),
            Node::Tag {
                protected_html: Some(full),
                ..
            } => full.clone(),
            Node::Tag { raw_inner, .. } => format!("<{raw_inner}>"),
        }
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        let Node::Tag { attrs, .. } = self else {
            return None;
        };
        attrs.iter().find_map(|a| {
            if a.name.eq_ignore_ascii_case(key) {
                a.value.as_deref()
            } else {
                None
            }
        })
    }

    pub fn set_attr(&mut self, key: &str, value: &str) {
        let Node::Tag {
            raw_inner, attrs, ..
        } = self
        else {
            return;
        };
        if let Some(a) = attrs
            .iter_mut()
            .find(|a| a.name.eq_ignore_ascii_case(key))
        {
            a.value = Some(value.to_string());
        } else {
            attrs.push(Attr {
                name: key.to_string(),
                value: Some(value.to_string()),
            });
        }
        *raw_inner = replace_attr_value(raw_inner, key, value);
    }
}

fn replace_attr_value(raw_inner: &str, key: &str, new_val: &str) -> String {
    let bytes = raw_inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let name = &raw_inner[start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let val_start = i;
            let (val_end, quote) = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                (i, Some(q as char))
            } else {
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
                    i += 1;
                }
                (i, None)
            };
            if name.eq_ignore_ascii_case(key) {
                let mut out = String::new();
                out.push_str(&raw_inner[..val_start]);
                match quote {
                    Some(q) => {
                        out.push(q);
                        out.push_str(new_val);
                        out.push(q);
                    }
                    None => {
                        if new_val.chars().any(|c| c.is_whitespace()) {
                            out.push('"');
                            out.push_str(new_val);
                            out.push('"');
                        } else {
                            out.push_str(new_val);
                        }
                    }
                }
                out.push_str(&raw_inner[val_end..]);
                return out;
            }
        }
    }
    format!("{raw_inner} {key}=\"{new_val}\"")
}

pub fn tokenize(input: &str, collapse_protected: bool) -> Vec<Node> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if input[i..].starts_with("<!--") {
                if let Some(rel) = input[i + 4..].find("-->") {
                    let end = i + 4 + rel + 3;
                    out.push(Node::Remark {
                        raw: input[i..end].to_string(),
                    });
                    i = end;
                    continue;
                }
            }
            if input[i..].len() >= 2 && input.as_bytes()[i + 1] == b'?' {
                if let Some(rel) = input[i + 2..].find("?>") {
                    let end = i + 2 + rel + 2;
                    out.push(Node::Text {
                        raw: input[i..end].to_string(),
                    });
                    i = end;
                    continue;
                }
            }
            if let Some((node, end)) = parse_tag(input, i, collapse_protected) {
                out.push(node);
                i = end;
                continue;
            }
        }
        let start = i;
        i += 1;
        while i < bytes.len() && bytes[i] != b'<' {
            i += 1;
        }
        out.push(Node::Text {
            raw: input[start..i].to_string(),
        });
    }
    out
}

fn parse_tag(input: &str, start: usize, collapse_protected: bool) -> Option<(Node, usize)> {
    if start >= input.len() || input.as_bytes()[start] != b'<' {
        return None;
    }
    let mut i = start + 1;
    let bytes = input.as_bytes();
    if i >= bytes.len() {
        return None;
    }
    let end_tag = bytes[i] == b'/';
    if end_tag {
        i += 1;
    }
    let name_start = i;
    if i < bytes.len() && bytes[i] == b'!' {
        i += 1;
    }
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b':')
    {
        i += 1;
    }
    if i == name_start && !(name_start < bytes.len() && bytes[name_start] == b'!') {
        return None;
    }
    let name = input[name_start..i].to_ascii_uppercase();
    // declarations / doctype: read until '>'
    if name.starts_with('!') || name == "!DOCTYPE" || input[name_start..].starts_with('!') {
        if let Some(rel) = input[start + 1..].find('>') {
            let end = start + 1 + rel + 1;
            let raw_inner = input[start + 1..end - 1].to_string();
            let tag_name = if raw_inner.to_ascii_uppercase().starts_with("!DOCTYPE") {
                "!DOCTYPE".to_string()
            } else {
                name
            };
            return Some((
                Node::Tag {
                    name: tag_name,
                    raw_inner,
                    attrs: vec![],
                    end_tag: false,
                    empty_xml: false,
                    protected_html: None,
                },
                end,
            ));
        }
        return None;
    }
    if name.is_empty() {
        return None;
    }
    let (raw_inner, gt, attrs, empty_xml) = parse_tag_tail(input, start, i)?;
    let mut end = gt + 1;
    let mut protected_html = None;
    if collapse_protected
        && !end_tag
        && matches!(name.as_str(), "SCRIPT" | "STYLE" | "OBJECT" | "EMBED")
    {
        if let Some(close) = find_end_tag(input, end, &name) {
            protected_html = Some(input[start..close].to_string());
            end = close;
        }
    }
    Some((
        Node::Tag {
            name,
            raw_inner,
            attrs,
            end_tag,
            empty_xml,
            protected_html,
        },
        end,
    ))
}

fn parse_tag_tail(input: &str, start: usize, mut i: usize) -> Option<(String, usize, Vec<Attr>, bool)> {
    let bytes = input.as_bytes();
    let mut attrs = Vec::new();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        if bytes[i] == b'>' {
            let raw_inner = input[start + 1..i].to_string();
            let empty_xml = raw_inner.trim_end().ends_with('/');
            return Some((raw_inner, i, attrs, empty_xml));
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
            let raw_inner = input[start + 1..i + 1].to_string();
            return Some((raw_inner, i + 1, attrs, true));
        }
        let an_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if i == an_start {
            i += 1;
            continue;
        }
        let aname = input[an_start..i].to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut aval = None;
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let q = bytes[i];
                i += 1;
                let vs = i;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                aval = Some(input[vs..i].to_string());
                if i < bytes.len() {
                    i += 1;
                }
            } else {
                let vs = i;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && bytes[i] != b'>'
                    && bytes[i] != b'/'
                {
                    i += 1;
                }
                aval = Some(input[vs..i].to_string());
            }
        }
        attrs.push(Attr {
            name: aname,
            value: aval,
        });
    }
}

fn find_end_tag(input: &str, from: usize, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let rest = &input[from..];
    let lower = rest.to_ascii_lowercase();
    let nlow = needle.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find(&nlow) {
        let at = from + search + rel;
        let after = at + needle.len();
        let bytes = input.as_bytes();
        if after < bytes.len() {
            let c = bytes[after];
            if c == b'>' || c.is_ascii_whitespace() || c == b'/' {
                if let Some(gt) = input[after..].find('>') {
                    return Some(after + gt + 1);
                }
            }
        } else if after == bytes.len() {
            return Some(after);
        }
        search += rel + 1;
    }
    None
}
