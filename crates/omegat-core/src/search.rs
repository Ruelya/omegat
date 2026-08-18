use crate::session::Entry;
use omegat_ipc::{SearchHitDto, SearchParams};
use regex::Regex;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Exact,
    Keyword,
    Regex,
}

pub fn search(entries: &[Entry], params: &SearchParams) -> Vec<SearchHitDto> {
    let kind = search_kind(params);
    let re = compile_regex(params, kind);
    let mut hits = Vec::new();
    for (index, e) in entries.iter().enumerate() {
        if !entry_passes_filters(e, params) {
            continue;
        }
        push_field(&mut hits, index, e, "source", &e.source, params.source, params, kind, re.as_ref());
        push_field(
            &mut hits,
            index,
            e,
            "translation",
            &e.translation,
            params.translation,
            params,
            kind,
            re.as_ref(),
        );
        push_field(&mut hits, index, e, "notes", &e.note, params.notes, params, kind, re.as_ref());
        push_field(
            &mut hits,
            index,
            e,
            "comments",
            &e.comment,
            params.comments,
            params,
            kind,
            re.as_ref(),
        );
    }
    hits
}

pub fn replace(entries: &mut [Entry], params: &SearchParams) -> usize {
    if params.preview {
        return 0;
    }
    let Some(repl) = &params.replace else {
        return 0;
    };
    let kind = search_kind(params);
    let re = compile_regex(params, kind);
    let mut n = 0;
    for e in entries.iter_mut() {
        if !entry_passes_filters(e, params) {
            continue;
        }
        let mut changed = false;
        if params.translation {
            if let Some(next) = replace_in(&e.translation, params, kind, re.as_ref(), repl) {
                e.translation = next;
                changed = true;
            }
        }
        if params.notes {
            if let Some(next) = replace_in(&e.note, params, kind, re.as_ref(), repl) {
                e.note = next;
                changed = true;
            }
        }
        if changed {
            e.revision += 1;
            n += 1;
        }
    }
    n
}

fn search_kind(params: &SearchParams) -> Kind {
    let ty = params.search_type.as_deref().unwrap_or("");
    if params.regex || ty.eq_ignore_ascii_case("regex") {
        Kind::Regex
    } else if ty.eq_ignore_ascii_case("keyword") {
        Kind::Keyword
    } else {
        Kind::Exact
    }
}

fn compile_regex(params: &SearchParams, kind: Kind) -> Option<Regex> {
    if kind != Kind::Regex || params.query.is_empty() {
        return None;
    }
    let mut pat = params.query.clone();
    if params.whole_word {
        pat = format!(r"\b(?:{pat})\b");
    }
    let flags = if params.case_sensitive { "" } else { "(?i)" };
    Regex::new(&format!("{flags}{pat}")).ok()
}

fn entry_passes_filters(e: &Entry, params: &SearchParams) -> bool {
    if params.untranslated && e.translated() {
        return false;
    }
    if let Some(author) = params.author.as_deref().filter(|s| !s.is_empty()) {
        let id = prop(e, "changeid").unwrap_or("");
        if !id.to_lowercase().contains(&author.to_lowercase()) {
            return false;
        }
    }
    if let Some(from) = params.date_from.as_deref().filter(|s| !s.is_empty()) {
        if prop(e, "changedate").unwrap_or("").as_bytes() < from.as_bytes() {
            return false;
        }
    }
    if let Some(to) = params.date_to.as_deref().filter(|s| !s.is_empty()) {
        if prop(e, "changedate").unwrap_or("\u{10ffff}").as_bytes() > to.as_bytes() {
            return false;
        }
    }
    true
}

fn prop<'a>(e: &'a Entry, key: &str) -> Option<&'a str> {
    e.properties
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn push_field(
    hits: &mut Vec<SearchHitDto>,
    index: usize,
    e: &Entry,
    field: &str,
    text: &str,
    enabled: bool,
    params: &SearchParams,
    kind: Kind,
    re: Option<&Regex>,
) {
    if !enabled || !field_matches(text, params, kind, re) {
        return;
    }
    let preview = if params.preview {
        params
            .replace
            .as_ref()
            .and_then(|repl| replace_in(text, params, kind, re, repl))
    } else {
        None
    };
    hits.push(SearchHitDto {
        index,
        file: e.file.clone(),
        field: field.into(),
        text: text.to_string(),
        preview,
    });
}

fn field_matches(text: &str, params: &SearchParams, kind: Kind, re: Option<&Regex>) -> bool {
    if params.query.is_empty() {
        return false;
    }
    match kind {
        Kind::Regex => re.is_some_and(|r| r.is_match(text)),
        Kind::Keyword => {
            let words: Vec<String> = params
                .query
                .split_whitespace()
                .map(|w| normalize_needle(w, params.case_sensitive))
                .collect();
            if words.is_empty() {
                return false;
            }
            let hay = normalize_hay(text, params.case_sensitive);
            words.iter().all(|w| contains_word(&hay, w, params.whole_word))
        }
        Kind::Exact => {
            let needle = normalize_needle(&params.query, params.case_sensitive);
            let hay = normalize_hay(text, params.case_sensitive);
            contains_word(&hay, &needle, params.whole_word)
        }
    }
}

fn replace_in(
    text: &str,
    params: &SearchParams,
    kind: Kind,
    re: Option<&Regex>,
    repl: &str,
) -> Option<String> {
    if !field_matches(text, params, kind, re) {
        return None;
    }
    let next = match kind {
        Kind::Regex => re.map(|r| r.replace_all(text, repl).into_owned())?,
        Kind::Keyword | Kind::Exact => {
            if params.case_sensitive {
                if params.whole_word {
                    replace_whole_word(text, &params.query, repl, true)
                } else {
                    text.replace(&params.query, repl)
                }
            } else if params.whole_word {
                replace_whole_word(text, &params.query, repl, false)
            } else {
                replace_ignore_case(text, &params.query, repl)
            }
        }
    };
    if next == text {
        None
    } else {
        Some(next)
    }
}

fn normalize_needle(s: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        s.to_string()
    } else {
        s.to_lowercase()
    }
}

fn normalize_hay(s: &str, case_sensitive: bool) -> String {
    normalize_needle(s, case_sensitive)
}

fn contains_word(hay: &str, needle: &str, whole: bool) -> bool {
    if needle.is_empty() {
        return false;
    }
    if !whole {
        return hay.contains(needle);
    }
    hay.split(|c: char| !c.is_alphanumeric())
        .any(|w| w == needle)
}

fn replace_ignore_case(text: &str, query: &str, repl: &str) -> String {
    let lower_q = query.to_lowercase();
    let mut out = String::new();
    let lower = text.to_lowercase();
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&lower_q) {
        let abs = i + pos;
        out.push_str(&text[i..abs]);
        out.push_str(repl);
        i = abs + query.len().min(text.len() - abs);
        if query.is_empty() {
            break;
        }
    }
    out.push_str(&text[i..]);
    out
}

fn replace_whole_word(text: &str, query: &str, repl: &str, case_sensitive: bool) -> String {
    let mut out = String::new();
    let mut last = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (byte, ch) = chars[i];
        if !ch.is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = byte;
        let mut j = i;
        while j < chars.len() && chars[j].1.is_alphanumeric() {
            j += 1;
        }
        let end = if j < chars.len() {
            chars[j].0
        } else {
            text.len()
        };
        let word = &text[start..end];
        let hit = if case_sensitive {
            word == query
        } else {
            word.eq_ignore_ascii_case(query)
        };
        if hit {
            out.push_str(&text[last..start]);
            out.push_str(repl);
            last = end;
        }
        i = j;
    }
    out.push_str(&text[last..]);
    out
}

/// Java `SearchExpression` + `Searcher.searchString` / replace matches.
#[derive(Debug, Clone)]
pub struct SearchExpression {
    pub query: String,
    pub kind: SearchKind,
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub width_insensitive: bool,
    pub replacement: Option<String>,
    pub author: Option<String>,
    pub search_author: bool,
    pub search_comments: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Exact,
    Keyword,
    Regex,
}

impl SearchExpression {
    pub fn exact(query: &str, case_sensitive: bool) -> Self {
        Self {
            query: query.into(),
            kind: SearchKind::Exact,
            case_sensitive,
            whole_words: false,
            width_insensitive: false,
            replacement: None,
            author: None,
            search_author: false,
            search_comments: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub replacement: String,
}

fn fold_width_spaces(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{00A0}' | '\u{2007}' | '\u{2009}' | '\u{202F}' | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

/// Java `StaticUtils.globToRegex` without `\Q`/`\E` (Rust regex has no quoting).
pub fn glob_to_regex(text: &str, space_match_nbsp: bool) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '*' => out.push_str(if space_match_nbsp {
                r"[^\s\u{00A0}]*"
            } else {
                r"\S*"
            }),
            '?' => out.push_str(if space_match_nbsp {
                r"[^\s\u{00A0}]"
            } else {
                r"\S"
            }),
            ' ' if space_match_nbsp => out.push_str("(?: |\u{00A0})"),
            c if r".+()[]{}|^$\\".contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

fn compile_search_regex(expr: &SearchExpression, needle: &str) -> Option<Regex> {
    let pat = match expr.kind {
        SearchKind::Regex => {
            let mut p = needle.to_string();
            if needle.contains(' ') {
                p = p.replace(' ', "( |\u{00A0})");
            }
            p
        }
        SearchKind::Exact => glob_to_regex(needle, false),
        SearchKind::Keyword => return None,
    };
    let flags = if expr.case_sensitive { "" } else { "(?i)" };
    Regex::new(&format!("{flags}{pat}")).ok()
}

fn is_java_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn regex_hits(re: &Regex, hay: &str, whole_words: bool) -> bool {
    for m in re.find_iter(hay) {
        if !whole_words {
            return true;
        }
        let before_ok = hay[..m.start()]
            .chars()
            .next_back()
            .map(|c| !is_java_word_char(c))
            .unwrap_or(true);
        let after_ok = hay[m.end()..]
            .chars()
            .next()
            .map(|c| !is_java_word_char(c))
            .unwrap_or(true);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

pub fn search_string(text: &str, expr: &SearchExpression) -> bool {
    if text.is_empty() {
        return false;
    }
    let hay = if expr.width_insensitive {
        fold_width_spaces(&crate::string_util::normalize_width(text))
    } else {
        text.to_string()
    };
    let needle = if expr.width_insensitive {
        fold_width_spaces(&crate::string_util::normalize_width(&expr.query))
    } else {
        expr.query.clone()
    };
    let whole = expr.whole_words && expr.kind != SearchKind::Regex;
    match expr.kind {
        SearchKind::Keyword => {
            let words: Vec<String> = needle.split(' ').filter(|w| !w.is_empty()).map(|w| w.to_string()).collect();
            words.iter().all(|w| {
                let glob = glob_to_regex(w, false);
                let flags = if expr.case_sensitive { "" } else { "(?i)" };
                Regex::new(&format!("{flags}{glob}"))
                    .ok()
                    .is_some_and(|r| regex_hits(&r, &hay, whole))
            })
        }
        _ => compile_search_regex(expr, &needle).is_some_and(|r| regex_hits(&r, &hay, whole)),
    }
}

pub fn search_replace_matches(text: &str, expr: &SearchExpression) -> Vec<SearchMatch> {
    let repl = expr.replacement.as_deref().unwrap_or("");
    let mut matches = Vec::new();
    match expr.kind {
        SearchKind::Regex => {
            let re = Regex::new(&format!(
                "{}{}",
                if expr.case_sensitive { "" } else { "(?i)" },
                expr.query
            ))
            .ok();
            if let Some(re) = re {
                for cap in re.captures_iter(text) {
                    let mut dest = String::new();
                    cap.expand(repl, &mut dest);
                    matches.push(SearchMatch { replacement: dest });
                }
            }
        }
        _ => {
            let hay = if expr.case_sensitive {
                text.to_string()
            } else {
                text.to_lowercase()
            };
            let needle = if expr.case_sensitive {
                expr.query.clone()
            } else {
                expr.query.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = hay[start..].find(&needle) {
                let abs = start + pos;
                matches.push(SearchMatch {
                    replacement: repl.to_string(),
                });
                start = abs + needle.len().max(1);
                if needle.is_empty() {
                    break;
                }
            }
        }
    }
    matches
}

#[derive(Debug, Default, Clone)]
pub struct SearchHit {
    pub src_text: String,
}

pub fn check_entry(
    source: &str,
    translation: Option<&str>,
    note: Option<&str>,
    comments: Option<&[&str]>,
    creator: Option<&str>,
    expr: &SearchExpression,
) -> Vec<SearchHit> {
    if expr.search_author {
        if let Some(want) = &expr.author {
            if creator != Some(want.as_str()) {
                return vec![];
            }
        }
    }
    let mut texts = vec![source.to_string()];
    if let Some(t) = translation {
        texts.push(t.to_string());
    }
    if let Some(n) = note {
        texts.push(n.to_string());
    }
    if expr.search_comments {
        if let Some(cs) = comments {
            for c in cs {
                texts.push((*c).to_string());
            }
        }
    }
    if texts.iter().any(|t| search_string(t, expr)) {
        vec![SearchHit {
            src_text: source.to_string(),
        }]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, translation: &str, note: &str) -> Entry {
        Entry {
            file: "a.txt".into(),
            id: "1".into(),
            source: source.into(),
            translation: translation.into(),
            note: note.into(),
            comment: "cmt".into(),
            default_translation: true,
            revision: 1,
            from_tm_exact: false,
            properties: vec![
                ("changeid".into(), "alice".into()),
                ("changedate".into(), "20200101T000000Z".into()),
            ],
        }
    }

    #[test]
    fn exact_source_and_notes() {
        let entries = vec![entry("Hello world", "Bonjour", "fix later")];
        let hits = search(
            &entries,
            &SearchParams {
                query: "Hello".into(),
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].field, "source");
        let notes = search(
            &entries,
            &SearchParams {
                query: "later".into(),
                source: false,
                translation: false,
                notes: true,
                ..Default::default()
            },
        );
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].field, "notes");
    }

    #[test]
    fn keyword_requires_all_words() {
        let entries = vec![entry("Hello brave world", "", "")];
        let miss = search(
            &entries,
            &SearchParams {
                query: "Hello missing".into(),
                search_type: Some("keyword".into()),
                translation: false,
                ..Default::default()
            },
        );
        assert!(miss.is_empty());
        let hit = search(
            &entries,
            &SearchParams {
                query: "Hello world".into(),
                search_type: Some("keyword".into()),
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(hit.len(), 1);
    }

    #[test]
    fn whole_word_and_case() {
        let entries = vec![entry("catalog cat", "Cat", "")];
        let loose = search(
            &entries,
            &SearchParams {
                query: "cat".into(),
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(loose.len(), 1);
        let whole = search(
            &entries,
            &SearchParams {
                query: "cat".into(),
                whole_word: true,
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(whole.len(), 1);
        let case = search(
            &entries,
            &SearchParams {
                query: "Cat".into(),
                case_sensitive: true,
                source: false,
                ..Default::default()
            },
        );
        assert_eq!(case.len(), 1);
        assert_eq!(case[0].field, "translation");
    }

    #[test]
    fn untranslated_author_date_and_preview() {
        let entries = vec![
            entry("Hello", "Bonjour", ""),
            entry("Goodbye", "", "todo"),
        ];
        let un = search(
            &entries,
            &SearchParams {
                query: "Good".into(),
                untranslated: true,
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(un.len(), 1);
        assert_eq!(un[0].index, 1);
        let author = search(
            &entries,
            &SearchParams {
                query: "Hello".into(),
                author: Some("alice".into()),
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(author.len(), 1);
        let dated = search(
            &entries,
            &SearchParams {
                query: "Hello".into(),
                date_from: Some("20190101".into()),
                date_to: Some("20210101".into()),
                translation: false,
                ..Default::default()
            },
        );
        assert_eq!(dated.len(), 1);
        let preview = search(
            &entries,
            &SearchParams {
                query: "todo".into(),
                source: false,
                translation: false,
                notes: true,
                replace: Some("done".into()),
                preview: true,
                ..Default::default()
            },
        );
        assert_eq!(preview[0].preview.as_deref(), Some("done"));
        let mut copy = entries.clone();
        let n = replace(
            &mut copy,
            &SearchParams {
                query: "todo".into(),
                notes: true,
                translation: false,
                replace: Some("done".into()),
                preview: true,
                ..Default::default()
            },
        );
        assert_eq!(n, 0);
        assert_eq!(copy[1].note, "todo");
        let n = replace(
            &mut copy,
            &SearchParams {
                query: "todo".into(),
                notes: true,
                translation: false,
                replace: Some("done".into()),
                ..Default::default()
            },
        );
        assert_eq!(n, 1);
        assert_eq!(copy[1].note, "done");
    }
}
