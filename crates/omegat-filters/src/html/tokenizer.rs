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
        matches!(self, Node::Tag { end_tag: true, .. })
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
        if let Some(a) = attrs.iter_mut().find(|a| a.name.eq_ignore_ascii_case(key)) {
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
    tokenize_with_protected(input, collapse_protected, |_| false)
}

/// Tokenize while allowing `FilterVisitor` options to protect arbitrary
/// composite elements. Java's visitor disables child traversal when an
/// opening tag matches `ignoreTags`; flattening the tree without collapsing
/// that element would otherwise leak its text as translatable segments.
pub fn tokenize_with_protected(
    input: &str,
    collapse_protected: bool,
    mut dynamically_protected: impl FnMut(&Node) -> bool,
) -> Vec<Node> {
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
                // HTMLParser treats a comment that reaches EOF as one remark.
                // Keeping it as a single node prevents malformed comment text
                // from leaking into the translation stream.
                out.push(Node::Remark {
                    raw: input[i..].to_string(),
                });
                break;
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
            if let Some((mut node, mut end)) = parse_tag(input, i) {
                let collapse = if let Node::Tag {
                    name,
                    end_tag,
                    empty_xml,
                    ..
                } = &node
                {
                    collapse_protected
                        && !end_tag
                        && !empty_xml
                        && (matches!(name.as_str(), "SCRIPT" | "STYLE" | "OBJECT" | "EMBED")
                            || dynamically_protected(&node))
                } else {
                    false
                };
                if collapse {
                    let name = node.tag_name().to_string();
                    let close = if matches!(name.as_str(), "SCRIPT" | "STYLE") {
                        find_raw_text_end_tag(input, end, &name)
                    } else {
                        find_matching_end_tag(input, end, &name)
                    };
                    if let Some(close) = close {
                        if let Node::Tag { protected_html, .. } = &mut node {
                            *protected_html = Some(input[i..close].to_string());
                        }
                        end = close;
                    } else {
                        // A raw-text or ignoreTags element is still protected
                        // when its end tag is missing. Browser/HTMLParser trees
                        // implicitly close it at EOF.
                        if let Node::Tag { protected_html, .. } = &mut node {
                            *protected_html = Some(input[i..].to_string());
                        }
                        end = input.len();
                    }
                }
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

fn parse_tag(input: &str, start: usize) -> Option<(Node, usize)> {
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
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b':')
    {
        i += 1;
    }
    if i == name_start && !(name_start < bytes.len() && bytes[name_start] == b'!') {
        return None;
    }
    let name = input[name_start..i].to_ascii_uppercase();
    // Declarations can contain quoted `>` characters and DOCTYPE internal
    // subsets. Stop only at a top-level, unquoted delimiter.
    if name.starts_with('!') || name == "!DOCTYPE" || input[name_start..].starts_with('!') {
        if let Some(gt) = find_declaration_end(input, start + 1) {
            let end = gt + 1;
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
    let end = gt + 1;
    Some((
        Node::Tag {
            name,
            raw_inner,
            attrs,
            end_tag,
            empty_xml,
            protected_html: None,
        },
        end,
    ))
}

fn find_declaration_end(input: &str, mut i: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut quote = None;
    let mut subset_depth = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'[' => subset_depth += 1,
                b']' => subset_depth = subset_depth.saturating_sub(1),
                b'>' if subset_depth == 0 => return Some(i),
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn parse_tag_tail(
    input: &str,
    start: usize,
    mut i: usize,
) -> Option<(String, usize, Vec<Attr>, bool)> {
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

fn find_matching_end_tag(input: &str, from: usize, name: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = from;
    while cursor < input.len() {
        let rel = input[cursor..].find('<')?;
        let at = cursor + rel;
        if let Some((node, end)) = parse_tag(input, at) {
            if node.tag_name().eq_ignore_ascii_case(name) {
                if node.is_end_tag() {
                    depth -= 1;
                    if depth == 0 {
                        return Some(end);
                    }
                } else if !node.is_empty_xml() {
                    if depth == 1 && implicitly_closes(name, &node) {
                        return Some(at);
                    }
                    depth += 1;
                }
            } else if depth == 1 && implicitly_closes(name, &node) {
                return Some(at);
            }
            cursor = end;
        } else {
            cursor = at + 1;
        }
    }
    None
}

fn find_raw_text_end_tag(input: &str, from: usize, name: &str) -> Option<usize> {
    let mut cursor = from;
    while cursor < input.len() {
        let rel = input[cursor..].find('<')?;
        let at = cursor + rel;
        if let Some((node, end)) = parse_tag(input, at) {
            if node.is_end_tag() && node.tag_name().eq_ignore_ascii_case(name) {
                return Some(end);
            }
            cursor = end;
        } else {
            cursor = at + 1;
        }
    }
    None
}

fn implicitly_closes(open: &str, candidate: &Node) -> bool {
    let name = candidate.tag_name();
    if candidate.is_end_tag() {
        return match open {
            "P" => matches!(
                name,
                "ADDRESS"
                    | "ARTICLE"
                    | "ASIDE"
                    | "BLOCKQUOTE"
                    | "BODY"
                    | "DIV"
                    | "DL"
                    | "FIELDSET"
                    | "FOOTER"
                    | "FORM"
                    | "H1"
                    | "H2"
                    | "H3"
                    | "H4"
                    | "H5"
                    | "H6"
                    | "HEADER"
                    | "HTML"
                    | "MAIN"
                    | "NAV"
                    | "OL"
                    | "PRE"
                    | "SECTION"
                    | "TABLE"
                    | "UL"
            ),
            "LI" => matches!(name, "OL" | "UL" | "MENU"),
            "DT" | "DD" => name == "DL",
            "RT" | "RP" => name == "RUBY",
            "OPTION" => matches!(name, "SELECT" | "DATALIST" | "OPTGROUP"),
            "OPTGROUP" => name == "SELECT",
            "THEAD" | "TBODY" | "TFOOT" => name == "TABLE",
            "TR" => matches!(name, "TABLE" | "THEAD" | "TBODY" | "TFOOT"),
            "TD" | "TH" => matches!(name, "TR" | "TABLE" | "THEAD" | "TBODY" | "TFOOT"),
            _ => false,
        };
    }
    match open {
        "P" => matches!(
            name,
            "ADDRESS"
                | "ARTICLE"
                | "ASIDE"
                | "BLOCKQUOTE"
                | "DIV"
                | "DL"
                | "FIELDSET"
                | "FOOTER"
                | "FORM"
                | "H1"
                | "H2"
                | "H3"
                | "H4"
                | "H5"
                | "H6"
                | "HEADER"
                | "HR"
                | "MAIN"
                | "NAV"
                | "OL"
                | "P"
                | "PRE"
                | "SECTION"
                | "TABLE"
                | "UL"
        ),
        "LI" => name == "LI",
        "DT" | "DD" => matches!(name, "DT" | "DD"),
        "RT" | "RP" => matches!(name, "RT" | "RP"),
        "OPTION" => matches!(name, "OPTION" | "OPTGROUP"),
        "OPTGROUP" => name == "OPTGROUP",
        "THEAD" => matches!(name, "TBODY" | "TFOOT"),
        "TBODY" => matches!(name, "TBODY" | "TFOOT"),
        "TR" => name == "TR",
        "TD" | "TH" => matches!(name, "TD" | "TH"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_protection_collapses_nested_matching_elements() {
        let raw =
            r#"<div class="notrans">secret<div class="notrans">nested</div>tail</div><p>shown</p>"#;
        let nodes =
            tokenize_with_protected(raw, true, |node| node.attr("class") == Some("notrans"));
        assert_eq!(nodes.len(), 4);
        assert_eq!(
            nodes[0].to_html(),
            r#"<div class="notrans">secret<div class="notrans">nested</div>tail</div>"#
        );
        assert_eq!(nodes[2].to_html(), "shown");
    }

    #[test]
    fn unterminated_protected_element_is_collapsed_through_eof() {
        let raw = r#"<p>shown</p><script>const hidden = "<p>not text";"#;
        let nodes = tokenize_with_protected(raw, true, |_| false);
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[3].to_html(), r#"<script>const hidden = "<p>not text";"#);
    }

    #[test]
    fn unterminated_dynamic_element_is_collapsed_through_eof() {
        let raw = r#"<div data-i18n="off">secret <b>still secret"#;
        let nodes =
            tokenize_with_protected(raw, true, |node| node.attr("data-i18n") == Some("off"));
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].to_html(), raw);
    }

    #[test]
    fn raw_text_closes_at_the_first_matching_end_tag() {
        let raw = r#"<script>const fake = "<script>";</script><p>shown</p>"#;
        let nodes = tokenize(raw, true);
        assert_eq!(nodes.len(), 4);
        assert_eq!(
            nodes[0].to_html(),
            r#"<script>const fake = "<script>";</script>"#
        );
        assert_eq!(nodes[2].to_html(), "shown");
    }

    #[test]
    fn optional_end_tag_stops_dynamic_protection_at_implicit_close() {
        let raw = r#"<p class="notrans">hidden<p>shown</p>"#;
        let nodes =
            tokenize_with_protected(raw, true, |node| node.attr("class") == Some("notrans"));
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].to_html(), r#"<p class="notrans">hidden"#);
        assert_eq!(nodes[1].to_html(), "<p>");
        assert_eq!(nodes[2].to_html(), "shown");
        assert_eq!(nodes[3].to_html(), "</p>");
    }

    #[test]
    fn optional_list_item_stops_at_parent_or_next_item() {
        let raw = r#"<ul><li class="notrans">one<li>two</ul>"#;
        let nodes =
            tokenize_with_protected(raw, true, |node| node.attr("class") == Some("notrans"));
        assert_eq!(
            nodes.iter().map(Node::to_html).collect::<Vec<_>>(),
            vec!["<ul>", r#"<li class="notrans">one"#, "<li>", "two", "</ul>",]
        );
    }

    #[test]
    fn unterminated_comment_remains_one_remark() {
        let raw = "<!-- hidden <p>still hidden";
        let nodes = tokenize(raw, true);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], Node::Remark { .. }));
        assert_eq!(nodes[0].to_html(), raw);
    }

    #[test]
    fn doctype_internal_subset_keeps_quoted_delimiters() {
        let raw = r#"<!DOCTYPE html [<!ENTITY sample "a > b">]><p>shown</p>"#;
        let nodes = tokenize(raw, true);
        assert_eq!(
            nodes[0].to_html(),
            r#"<!DOCTYPE html [<!ENTITY sample "a > b">]>"#
        );
        assert_eq!(nodes[2].to_html(), "shown");
    }
}
