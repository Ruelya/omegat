//! Java `org.omegat.filters2.html2.FilterVisitor` + HHC override.

use super::html_options::HtmlOptions;
use super::html_writer::{
    chars_to_entities, compress_spaces, compress_whitespace_layout, entities_to_chars, java_trim,
    rewrite_encoding_header, space_postfix, space_prefix,
};
use super::tokenizer::{tokenize_with_protected, Node};
use crate::{ExtractedSegment, FilterContext, ParsedFile, ProtectedPart};
use regex::Regex;
use std::collections::HashMap;

const BLOCK_TAGS: &[&str] = &[
    "ADDRESS",
    "ARTICLE",
    "ASIDE",
    "BLOCKQUOTE",
    "BODY",
    "CANVAS",
    "CENTER",
    "DD",
    "DIV",
    "DL",
    "DT",
    "FIELDSET",
    "FIGCAPTION",
    "FIGURE",
    "FOOTER",
    "FORM",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "HEADER",
    "HR",
    "LABEL",
    "LEGEND",
    "LI",
    "MAIN",
    "NAV",
    "NOSCRIPT",
    "OL",
    "OPTION",
    "P",
    "PRE",
    "SECTION",
    "SELECT",
    "TABLE",
    "TD",
    "TEXTAREA",
    "TFOOT",
    "TH",
    "TITLE",
    "TR",
    "UL",
    "VIDEO",
];
const PARENT_TAGS: &[&str] = &["HEAD", "HTML"];
const PROTECTED_TAGS: &[&str] = &["!DOCTYPE", "STYLE", "SCRIPT", "OBJECT", "EMBED"];
const TRANSLATABLE_INPUT: &[&str] = &["submit", "button", "reset"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisitorKind {
    Html,
    Hhc,
}

pub struct HtmlOutcome {
    pub parsed: ParsedFile,
    pub written: String,
}

struct Visitor<'a> {
    kind: VisitorKind,
    options: &'a HtmlOptions,
    ctx: &'a FilterContext,
    encoding: String,
    skip_re: Option<Regex>,
    process: Box<dyn FnMut(&str, Option<&str>) -> String + 'a>,
    sources: Vec<String>,
    comments: Vec<Option<String>>,
    out: String,
    recurse_children: bool,
    collecting: bool,
    pre: bool,
    preceding: Vec<Node>,
    translatable: Vec<Node>,
    following: Vec<Node>,
    s_tags: Vec<Node>,
    s_nums: Vec<i32>,
    s_shortcuts: Vec<String>,
    s_n: i32,
    firstcall: bool,
}

impl<'a> Visitor<'a> {
    fn new(
        kind: VisitorKind,
        options: &'a HtmlOptions,
        ctx: &'a FilterContext,
        process: Box<dyn FnMut(&str, Option<&str>) -> String + 'a>,
    ) -> Self {
        let skip_re = if options.skip_regexp.trim().is_empty() {
            None
        } else {
            Regex::new(&format!("(?i){}", options.skip_regexp)).ok()
        };
        Self {
            kind,
            options,
            ctx,
            encoding: "UTF-8".into(),
            skip_re,
            process,
            sources: Vec::new(),
            comments: Vec::new(),
            out: String::new(),
            recurse_children: true,
            collecting: false,
            pre: false,
            preceding: Vec::new(),
            translatable: Vec::new(),
            following: Vec::new(),
            s_tags: Vec::new(),
            s_nums: Vec::new(),
            s_shortcuts: Vec::new(),
            s_n: 0,
            firstcall: true,
        }
    }

    fn process_entry(&mut self, entry: &str, comment: Option<&str>) -> String {
        if let Some(re) = &self.skip_re {
            if re.is_match(entry) {
                return entry.to_string();
            }
        }
        if !entry.is_empty() {
            self.sources.push(entry.to_string());
            self.comments.push(comment.map(|s| s.to_string()));
        }
        (self.process)(entry, comment)
    }

    fn writeout(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn is_paragraph(&self, tag: &Node) -> bool {
        let name = tag.tag_name();
        if self.kind == VisitorKind::Hhc {
            return matches!(name, "HTML" | "HEAD" | "BODY");
        }
        (name == "BR" && self.options.paragraph_on_br)
            || PARENT_TAGS.contains(&name)
            || BLOCK_TAGS.contains(&name)
    }

    fn is_protected(&self, tag: &Node) -> bool {
        if self.kind == VisitorKind::Hhc {
            return false;
        }
        let name = tag.tag_name();
        if PROTECTED_TAGS.contains(&name) {
            return true;
        }
        if name == "META"
            && tag
                .attr("http-equiv")
                .is_some_and(|v| v.eq_ignore_ascii_case("content-type"))
        {
            return true;
        }
        self.has_ignore_attrs(tag)
    }

    fn has_ignore_attrs(&self, tag: &Node) -> bool {
        let Node::Tag { attrs, .. } = tag else {
            return false;
        };
        for a in attrs {
            if let Some(v) = &a.value {
                for (k, pv) in self.options.ignore_tag_pairs() {
                    if a.name.eq_ignore_ascii_case(&k) && v.eq_ignore_ascii_case(&pv) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn is_pre(&self, tag: &Node) -> bool {
        matches!(tag.tag_name(), "PRE" | "TEXTAREA")
    }

    fn skip_meta(&self, tag: &Node) -> bool {
        let Node::Tag { attrs, .. } = tag else {
            return false;
        };
        for a in attrs {
            if let Some(v) = &a.value {
                for (k, pv) in self.options.skip_meta_pairs() {
                    if a.name.eq_ignore_ascii_case(&k) && v.eq_ignore_ascii_case(&pv) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn maybe_translate_attr(&mut self, tag: &mut Node, key: &str, comment: Option<String>) {
        let Some(attr) = tag.attr(key).map(|s| s.to_string()) else {
            return;
        };
        let src = entities_to_chars(&attr);
        let trans = self.process_entry(&src, comment.as_deref());
        let enc = chars_to_entities(&trans, Some(&self.encoding), &self.s_shortcuts);
        tag.set_attr(key, &enc);
    }

    fn translate_std_attrs(&mut self, tag: &mut Node) {
        if self.kind == VisitorKind::Hhc {
            if tag.tag_name() == "PARAM"
                && tag
                    .attr("name")
                    .is_some_and(|v| v.eq_ignore_ascii_case("Name"))
            {
                self.maybe_translate_attr(tag, "value", None);
            }
            return;
        }
        let name = tag.tag_name().to_string();
        self.maybe_translate_attr(tag, "abbr", Some(attr_comment(&name, "abbr")));
        self.maybe_translate_attr(tag, "alt", Some(attr_comment(&name, "alt")));
        self.maybe_translate_attr(tag, "dir", Some(attr_comment(&name, "dir")));
        if self.options.translate_href {
            self.maybe_translate_attr(tag, "href", Some(attr_comment(&name, "href")));
        }
        if self.options.translate_hreflang {
            self.maybe_translate_attr(tag, "hreflang", Some(attr_comment(&name, "hreflang")));
        }
        if self.options.translate_lang {
            self.maybe_translate_attr(tag, "lang", Some(attr_comment(&name, "lang")));
            self.maybe_translate_attr(tag, "xml:lang", Some(attr_comment(&name, "xml:lang")));
        }
        self.maybe_translate_attr(tag, "label", Some(attr_comment(&name, "label")));
        if name == "IMG" && self.options.translate_src {
            self.maybe_translate_attr(tag, "src", Some(attr_comment(&name, "src")));
        }
        self.maybe_translate_attr(tag, "summary", Some(attr_comment(&name, "summary")));
        self.maybe_translate_attr(tag, "title", Some(attr_comment(&name, "title")));
        if name == "INPUT" {
            if self.is_translate_input_value(tag) {
                self.maybe_translate_attr(tag, "value", Some(attr_comment(&name, "value")));
            }
            self.maybe_translate_attr(tag, "placeholder", Some(attr_comment(&name, "placeholder")));
        }
        if name == "META" && !self.skip_meta(tag) {
            self.maybe_translate_attr(tag, "content", Some(attr_comment(&name, "content")));
        }
    }

    fn is_translate_input_value(&self, tag: &Node) -> bool {
        if !self.options.translate_value && !self.options.translate_button_value {
            return false;
        }
        tag.attr("type")
            .map(|t| TRANSLATABLE_INPUT.contains(&t.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn visit_tag(&mut self, mut tag: Node) {
        if self.kind == VisitorKind::Hhc {
            if self.is_paragraph(&tag) && self.collecting {
                self.endup();
            }
            self.translate_std_attrs(&mut tag);
            self.queue_prefix_tag(tag);
            return;
        }
        if self.is_protected(&tag) {
            if self.collecting {
                self.endup();
            } else {
                self.write_preceding();
            }
            self.writeout(&tag.to_html());
            return;
        }
        if self.is_paragraph(&tag) {
            self.recurse_children = true;
            if self.collecting {
                self.endup();
            }
        }
        if self.is_pre(&tag) {
            self.pre = true;
        }
        self.translate_std_attrs(&mut tag);
        self.queue_prefix_tag(tag);
    }

    fn visit_string(&mut self, text: Node) {
        self.recurse_children = true;
        let cleaned = entities_to_chars(text.get_text()).replace('\u{00A0}', " ");
        if !java_trim(&cleaned).is_empty() {
            if self.firstcall && is_xml_header(java_trim(&cleaned)) {
                self.writeout(&text.to_html());
                return;
            }
            self.collecting = true;
            self.firstcall = false;
        } else if self.pre {
            self.collecting = true;
        }
        if self.collecting {
            self.queue_translatable_text(text);
        } else {
            self.preceding.push(text);
        }
    }

    fn visit_remark(&mut self, remark: Node) {
        if self.options.remove_comments {
            return;
        }
        self.recurse_children = true;
        if self.pre {
            self.collecting = true;
        }
        if self.collecting {
            if self.pre {
                self.translatable.append(&mut self.following);
                self.translatable.push(remark);
            } else {
                self.following.push(remark);
            }
        } else {
            self.preceding.push(remark);
        }
    }

    fn visit_end(&mut self, tag: Node) {
        self.recurse_children = true;
        if self.is_paragraph(&tag) && self.collecting {
            self.endup();
        }
        if self.is_pre(&tag) {
            self.pre = false;
        }
        self.queue_prefix_tag(tag);
    }

    fn queue_translatable_text(&mut self, txt: Node) {
        if !java_trim(&txt.to_html()).is_empty() || self.pre {
            self.translatable.append(&mut self.following);
            self.translatable.push(txt);
        } else {
            self.following.push(txt);
        }
    }

    fn queue_prefix_tag(&mut self, tag: Node) {
        if self.collecting {
            self.following.push(tag);
        } else if self.is_paragraph(&tag) {
            self.write_preceding();
            self.writeout(&format!("<{}>", tag.get_text()));
        } else {
            self.preceding.push(tag);
        }
    }

    fn write_preceding(&mut self) {
        let nodes = std::mem::take(&mut self.preceding);
        for n in nodes {
            self.write_node_raw(&n);
        }
    }

    fn write_node_raw(&mut self, node: &Node) {
        match node {
            Node::Tag { .. } => self.writeout(&format!("<{}>", node.get_text())),
            Node::Remark { .. } => self.writeout(&node.to_html()),
            Node::Text { raw } => self.writeout(&compress_whitespace_layout(
                raw,
                self.options.compress_whitespace,
            )),
        }
    }

    fn endup(&mut self) {
        let mut all = Vec::new();
        all.append(&mut self.preceding);
        let last_prec = all.len() as i32 - 1;
        all.append(&mut self.translatable);
        let last_trans = all.len() as i32 - 1;
        all.append(&mut self.following);
        let last_follow = all.len() as i32 - 1;

        let mut first = 0i32;
        while first <= last_prec {
            if let Node::Tag { end_tag: false, .. } = &all[first as usize] {
                let opening = all[first as usize].tag_name().to_string();
                let mut rec = 1;
                let mut found = false;
                for i in (first + 1)..=last_trans {
                    if let Node::Tag { name, end_tag, .. } = &all[i as usize] {
                        if name == &opening {
                            if *end_tag {
                                rec -= 1;
                                if rec == 0 {
                                    if i > last_prec {
                                        found = true;
                                    }
                                    break;
                                }
                            } else {
                                rec += 1;
                            }
                        }
                    }
                }
                if found {
                    break;
                }
            }
            first += 1;
        }

        let mut last_keep = last_follow;
        while last_keep > last_trans {
            if let Node::Tag { end_tag: true, .. } = &all[last_keep as usize] {
                let closing = all[last_keep as usize].tag_name().to_string();
                let mut rec = 1;
                let mut found = false;
                let mut i = last_keep - 1;
                while i > last_prec {
                    if let Node::Tag { name, end_tag, .. } = &all[i as usize] {
                        if name == &closing {
                            if *end_tag {
                                rec += 1;
                            } else {
                                rec -= 1;
                                if rec == 0 {
                                    if i <= last_trans {
                                        found = true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    i -= 1;
                }
                if found {
                    break;
                }
            }
            last_keep -= 1;
        }

        let mut changed = true;
        while changed {
            changed = false;
            if !self.ctx.remove_tags {
                for i in 0..first {
                    if matches!(all[i as usize], Node::Tag { .. }) {
                        first = i;
                        changed = true;
                        break;
                    }
                }
                for i in (last_keep + 1..=last_follow).rev() {
                    if matches!(all[i as usize], Node::Tag { .. }) {
                        last_keep = i;
                        changed = true;
                        break;
                    }
                }
            }
            if !self.ctx.remove_spaces_nonseg {
                for i in 0..first {
                    if matches!(all[i as usize], Node::Text { .. }) {
                        first = i;
                        changed = true;
                        break;
                    }
                }
                for i in (last_keep + 1..=last_follow).rev() {
                    if matches!(all[i as usize], Node::Text { .. }) {
                        last_keep = i;
                        changed = true;
                        break;
                    }
                }
            }
        }

        for i in 0..first {
            if i >= 0 {
                self.write_node_raw(&all[i as usize]);
            }
        }

        let mut paragraph = String::new();
        if first <= last_keep {
            for i in first..=last_keep {
                match &all[i as usize] {
                    Node::Tag { .. } => self.assign_tag_shortcut(&all[i as usize], &mut paragraph),
                    Node::Remark { .. } => {
                        self.assign_remark_shortcut(&all[i as usize], &mut paragraph)
                    }
                    Node::Text { .. } => {
                        paragraph.push_str(&entities_to_chars(&all[i as usize].to_html()));
                    }
                }
            }
        }

        let uncompressed = paragraph;
        let mut space_pre = String::new();
        let mut space_post = String::new();
        let mut compressed = uncompressed.clone();
        if !self.pre {
            space_pre = space_prefix(&uncompressed, self.options.compress_whitespace);
            space_post = space_postfix(&uncompressed, self.options.compress_whitespace);
            if self.ctx.remove_spaces_nonseg {
                compressed = compress_spaces(&uncompressed);
            }
        }

        let mut translation = self.process_entry(&compressed, None);
        if compressed == translation && !self.options.compress_whitespace {
            translation = uncompressed;
            space_pre.clear();
            space_post.clear();
        }
        translation = chars_to_entities(&translation, Some(&self.encoding), &self.s_shortcuts);
        translation = self.revert_shortcut(&translation);
        self.writeout(&space_pre);
        self.writeout(&translation);
        self.writeout(&space_post);

        for i in (last_keep + 1)..=last_follow {
            if i >= 0 && (i as usize) < all.len() {
                self.write_node_raw(&all[i as usize]);
            }
        }
        self.cleanup();
    }

    fn assign_tag_shortcut(&mut self, tag: &Node, paragraph: &mut String) {
        let mut result = String::from("<");
        let mut n = -1;
        if tag.is_end_tag() {
            result.push('/');
            let mut rec = 1;
            for i in (0..self.s_tags.len()).rev() {
                if let Node::Tag { name, end_tag, .. } = &self.s_tags[i] {
                    if name == tag.tag_name() {
                        if *end_tag {
                            rec += 1;
                        } else {
                            rec -= 1;
                            if rec == 0 {
                                n = self.s_nums[i];
                                break;
                            }
                        }
                    }
                }
            }
            if n < 0 {
                n = self.s_n;
                self.s_n += 1;
            }
        } else {
            n = self.s_n;
            self.s_n += 1;
        }
        if tag.tag_name() == "BR" {
            result.push_str("br");
        } else if let Some(c) = tag.tag_name().chars().next() {
            result.push(c.to_ascii_lowercase());
        }
        result.push_str(&n.to_string());
        if tag.is_empty_xml() {
            result.push('/');
        }
        result.push('>');
        self.s_tags.push(tag.clone());
        self.s_nums.push(n);
        self.s_shortcuts.push(result.clone());
        paragraph.push_str(&result);
    }

    fn assign_remark_shortcut(&mut self, remark: &Node, paragraph: &mut String) {
        let n = self.s_n;
        self.s_n += 1;
        let shortcut = format!("<c{n}/>");
        self.s_tags.push(remark.clone());
        self.s_nums.push(n);
        self.s_shortcuts.push(shortcut.clone());
        paragraph.push_str(&shortcut);
    }

    fn revert_shortcut(&self, str_in: &str) -> String {
        let mut s = str_in.to_string();
        for (i, shortcut) in self.s_shortcuts.iter().enumerate() {
            let mut pos = 0;
            while let Some(found) = s[pos..].find(shortcut) {
                let at = pos + found;
                let repl = match &self.s_tags[i] {
                    Node::Tag { .. } => format!("<{}>", self.s_tags[i].get_text()),
                    Node::Remark { .. } => self.s_tags[i].to_html(),
                    Node::Text { raw } => raw.clone(),
                };
                s.replace_range(at..at + shortcut.len(), &repl);
                pos = at + repl.len();
                if pos >= s.len() {
                    break;
                }
            }
        }
        s
    }

    fn cleanup(&mut self) {
        self.collecting = false;
        self.recurse_children = true;
        self.preceding.clear();
        self.translatable.clear();
        self.following.clear();
        self.s_tags.clear();
        self.s_nums.clear();
        self.s_shortcuts.clear();
        self.s_n = 0;
    }

    fn finish(&mut self) {
        if self.collecting {
            self.endup();
        } else {
            self.write_preceding();
        }
    }
}

fn attr_comment(tag: &str, key: &str) -> String {
    format!("Tag {tag} Attribute {key}")
}

fn is_xml_header(s: &str) -> bool {
    Regex::new(r"^<\?xml.*?\?>$").unwrap().is_match(s)
}

pub fn process_html(
    raw: &str,
    ctx: &FilterContext,
    kind: VisitorKind,
    translations: Option<&HashMap<String, String>>,
) -> HtmlOutcome {
    let options = HtmlOptions::from_ctx(ctx);
    let lookup = translations.cloned().unwrap_or_default();
    let process = Box::new(move |entry: &str, _comment: Option<&str>| {
        lookup
            .get(entry)
            .cloned()
            .unwrap_or_else(|| entry.to_string())
    });
    let mut v = Visitor::new(kind, &options, ctx, process);
    let collapse = kind == VisitorKind::Html;
    let ignore_tag_pairs = options.ignore_tag_pairs();
    let nodes = tokenize_with_protected(raw, collapse, |node| {
        ignore_tag_pairs.iter().any(|(key, value)| {
            node.attr(key)
                .is_some_and(|actual| actual.eq_ignore_ascii_case(value))
        })
    });
    for node in nodes {
        match node {
            Node::Tag { end_tag: true, .. } => v.visit_end(node),
            Node::Tag {
                protected_html: Some(_),
                ..
            } => v.visit_tag(node),
            Node::Tag { .. } => v.visit_tag(node),
            Node::Text { .. } => v.visit_string(node),
            Node::Remark { .. } => v.visit_remark(node),
        }
    }
    v.finish();
    let written = rewrite_encoding_header(&v.out, &v.encoding, &options);
    let segments = v
        .sources
        .into_iter()
        .zip(v.comments.into_iter())
        .enumerate()
        .map(|(i, (source, comment))| ExtractedSegment {
            id: String::new(),
            source,
            existing_translation: None,
            note: None,
            comment,
            path: None,
            protected_parts: vec![ProtectedPart {
                text: format!("{i}"),
                details: String::new(),
            }]
            .into_iter()
            .filter(|_| false)
            .collect(),
        })
        .collect();
    HtmlOutcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(outcome: &HtmlOutcome) -> Vec<&str> {
        outcome
            .parsed
            .segments
            .iter()
            .map(|segment| segment.source.as_str())
            .collect()
    }

    #[test]
    fn ignore_tags_protects_the_entire_nested_subtree() {
        let raw = concat!(
            r#"<main><div class="notrans">secret "#,
            r#"<div class="notrans">nested</div> tail</div>"#,
            r#"<p title="heading">shown</p></main>"#
        );
        let mut ctx = FilterContext::default();
        ctx.options
            .insert("ignoreTags".into(), "class=notrans".into());
        let outcome = process_html(raw, &ctx, VisitorKind::Html, None);
        assert_eq!(sources(&outcome), vec!["heading", "shown"]);
        assert_eq!(outcome.written, raw);
    }

    #[test]
    fn ignored_subtree_stays_verbatim_during_other_translations() {
        let raw = r#"<div data-i18n="off"><b>do not translate</b></div><p>Hello</p>"#;
        let mut ctx = FilterContext::default();
        ctx.options
            .insert("ignoreTags".into(), "data-i18n=off".into());
        let translations = HashMap::from([("Hello".to_string(), "Bonjour".to_string())]);
        let outcome = process_html(raw, &ctx, VisitorKind::Html, Some(&translations));
        assert_eq!(sources(&outcome), vec!["Hello"]);
        assert_eq!(
            outcome.written,
            r#"<div data-i18n="off"><b>do not translate</b></div><p>Bonjour</p>"#
        );
    }
}
