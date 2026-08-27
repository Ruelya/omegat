use crate::session::Entry;
use omegat_ipc::{SearchHitDto, SearchParams};
use regex::Regex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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
        push_field(
            &mut hits,
            index,
            e,
            "source",
            &e.source,
            params.source,
            params,
            kind,
            re.as_ref(),
        );
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
        push_field(
            &mut hits,
            index,
            e,
            "notes",
            &e.note,
            params.notes,
            params,
            kind,
            re.as_ref(),
        );
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
            words
                .iter()
                .all(|w| contains_word(&hay, w, params.whole_word))
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
///
/// The IPC search path above intentionally accepts the compact wire DTO.  This
/// expression is the richer product model used by project search: it keeps the
/// source/target/property switches, duplicate policy, origin labels and
/// replacement mode together exactly as the Java controller does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchExpression {
    pub query: String,
    pub kind: SearchKind,
    pub mode: SearchMode,
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub width_insensitive: bool,
    pub space_match_nbsp: bool,
    pub replacement: Option<String>,
    pub author: Option<String>,
    pub search_author: bool,
    pub date_before: Option<i64>,
    pub date_after: Option<i64>,
    pub search_source: bool,
    pub search_target: bool,
    pub search_notes: bool,
    pub search_comments: bool,
    pub search_translated: bool,
    pub search_untranslated: bool,
    pub all_results: bool,
    pub file_names: bool,
    pub exclude_orphans: bool,
    pub number_of_results: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Exact,
    Keyword,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Search,
    Replace,
}

impl SearchExpression {
    pub fn exact(query: &str, case_sensitive: bool) -> Self {
        Self {
            query: query.into(),
            kind: SearchKind::Exact,
            mode: SearchMode::Search,
            case_sensitive,
            whole_words: false,
            width_insensitive: false,
            space_match_nbsp: false,
            replacement: None,
            author: None,
            search_author: false,
            date_before: None,
            date_after: None,
            search_source: true,
            search_target: true,
            search_notes: true,
            search_comments: true,
            search_translated: true,
            search_untranslated: true,
            all_results: true,
            file_names: true,
            exclude_orphans: false,
            number_of_results: 1000,
        }
    }

    pub fn keyword(query: &str, case_sensitive: bool) -> Self {
        Self {
            kind: SearchKind::Keyword,
            ..Self::exact(query, case_sensitive)
        }
    }

    pub fn regex(query: &str, case_sensitive: bool) -> Self {
        Self {
            kind: SearchKind::Regex,
            ..Self::exact(query, case_sensitive)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Java `Matcher.start()` offset, represented in UTF-16 code units.
    pub start: usize,
    /// Java `Matcher.end()` offset, represented in UTF-16 code units.
    pub end: usize,
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
            // Java `\\S` is `[^\t\n\x0B\f\r ]` — `\u00A0` is *not* whitespace.
            // Rust `\\S` is Unicode and would reject the `a*b` vs `a\u00A0b` case.
            '*' => out.push_str(if space_match_nbsp {
                r"[^\t\n\x0B\f\r \u{00A0}]*"
            } else {
                r"[^\t\n\x0B\f\r ]*"
            }),
            '?' => out.push_str(if space_match_nbsp {
                r"[^\t\n\x0B\f\r \u{00A0}]"
            } else {
                r"[^\t\n\x0B\f\r ]"
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
            if expr.space_match_nbsp && needle.contains(' ') {
                p = p.replace(' ', "( |\u{00A0})");
            }
            if expr.space_match_nbsp && needle.contains(r"\s") {
                p = p.replace(r"\s", r"(?:\s|\u{00A0})");
            }
            p
        }
        SearchKind::Exact => glob_to_regex(needle, expr.space_match_nbsp),
        SearchKind::Keyword => return None,
    };
    let flags = if expr.case_sensitive { "" } else { "(?i)" };
    Regex::new(&format!("{flags}{pat}")).ok()
}

fn is_java_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn has_word_boundaries(hay: &str, start: usize, end: usize) -> bool {
    let before_ok = hay[..start]
        .chars()
        .next_back()
        .map(|c| !is_java_word_char(c))
        .unwrap_or(true);
    let after_ok = hay[end..]
        .chars()
        .next()
        .map(|c| !is_java_word_char(c))
        .unwrap_or(true);
    before_ok && after_ok
}

fn normalized_search_text(text: &str, expr: &SearchExpression) -> String {
    if expr.width_insensitive {
        fold_width_spaces(&crate::string_util::normalize_width(text))
    } else {
        text.to_string()
    }
}

fn normalized_query(expr: &SearchExpression) -> String {
    normalized_search_text(&expr.query, expr)
}

fn utf16_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].encode_utf16().count()
}

fn replacement_for_capture(
    captures: &regex::Captures<'_>,
    expr: &SearchExpression,
    target_locale: &str,
) -> String {
    let Some(replacement) = expr.replacement.as_deref() else {
        return String::new();
    };
    if expr.kind == SearchKind::Regex {
        let mut expanded = String::new();
        captures.expand(replacement, &mut expanded);
        crate::string_util::replace_case(&expanded, target_locale)
    } else {
        replacement.to_string()
    }
}

fn compiled_matchers(expr: &SearchExpression) -> Option<Vec<Regex>> {
    let needle = normalized_query(expr);
    match expr.kind {
        SearchKind::Keyword => {
            let words: Vec<&str> = needle.split(' ').filter(|w| !w.is_empty()).collect();
            if words.is_empty() {
                return Some(Vec::new());
            }
            words
                .into_iter()
                .map(|word| {
                    let glob = glob_to_regex(word, false);
                    let flags = if expr.case_sensitive { "" } else { "(?i)" };
                    Regex::new(&format!("{flags}{glob}")).ok()
                })
                .collect()
        }
        _ => compile_search_regex(expr, &needle).map(|regex| vec![regex]),
    }
}

/// Return every concrete match region. Keyword searches require every keyword
/// to be present and then retain every region from every keyword matcher.
/// Search/replace always requests `collapse_results = false`, matching Java's
/// bug-675 behavior.
pub fn find_matches(
    text: Option<&str>,
    expr: &SearchExpression,
    collapse_results: bool,
    target_locale: &str,
) -> Vec<SearchMatch> {
    let Some(text) = text else {
        return Vec::new();
    };
    let hay = normalized_search_text(text, expr);
    let Some(matchers) = compiled_matchers(expr) else {
        return Vec::new();
    };
    if matchers.is_empty() {
        return Vec::new();
    }
    let use_word_boundaries = expr.whole_words && expr.kind != SearchKind::Regex;
    let mut found = Vec::new();
    for regex in matchers {
        let mut matcher_found = Vec::new();
        for captures in regex.captures_iter(&hay) {
            let Some(region) = captures.get(0) else {
                continue;
            };
            if use_word_boundaries && !has_word_boundaries(&hay, region.start(), region.end()) {
                continue;
            }
            // Java treats a zero-width match at a non-zero position as a hit,
            // but does not make it a replaceable/highlightable region.
            if region.start() == region.end() {
                continue;
            }
            matcher_found.push(SearchMatch {
                start: utf16_offset(&hay, region.start()),
                end: utf16_offset(&hay, region.end()),
                replacement: replacement_for_capture(&captures, expr, target_locale),
            });
        }
        if matcher_found.is_empty() {
            return Vec::new();
        }
        found.extend(matcher_found);
    }
    found.sort_by_key(|m| (m.start, m.end));
    if collapse_results {
        let mut collapsed: Vec<SearchMatch> = Vec::with_capacity(found.len());
        for current in found {
            if let Some(previous) = collapsed.last_mut() {
                if current.start <= previous.end {
                    previous.end = previous.end.max(current.end);
                    continue;
                }
            }
            collapsed.push(current);
        }
        collapsed
    } else {
        found
    }
}

pub fn search_string(text: &str, expr: &SearchExpression) -> bool {
    !find_matches(Some(text), expr, true, "und").is_empty()
}

pub fn search_replace_matches(text: &str, expr: &SearchExpression) -> Vec<SearchMatch> {
    find_matches(Some(text), expr, false, "und")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOrigin {
    Project { entry_number: usize, file: String },
    TranslationMemory { preamble: String },
    Orphan { preamble: String },
    Alternative { preamble: String },
    Glossary { preamble: String },
    Text { preamble: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSourceKind {
    ProjectFile,
    ExternalTranslationMemory,
    Glossary,
    TextFile,
}

/// One independently traversed project-search source.
///
/// Java's `Searcher` walks project files, external TMs, glossaries and loose
/// text in separate phases. Keeping these batches intact preserves origin
/// labels and provides cancellation/progress boundaries without requiring the
/// caller to flatten every source into one synthetic entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSource {
    pub kind: SearchSourceKind,
    pub label: String,
    pub entries: Vec<ProjectSearchEntry>,
}

impl SearchSource {
    pub fn project_file(label: impl Into<String>, entries: Vec<ProjectSearchEntry>) -> Self {
        Self {
            kind: SearchSourceKind::ProjectFile,
            label: label.into(),
            entries,
        }
    }

    pub fn external_tm(label: impl Into<String>, entries: Vec<ProjectSearchEntry>) -> Self {
        Self {
            kind: SearchSourceKind::ExternalTranslationMemory,
            label: label.into(),
            entries,
        }
    }

    pub fn glossary(label: impl Into<String>, entries: Vec<ProjectSearchEntry>) -> Self {
        Self {
            kind: SearchSourceKind::Glossary,
            label: label.into(),
            entries,
        }
    }

    pub fn text_file(label: impl Into<String>, entries: Vec<ProjectSearchEntry>) -> Self {
        Self {
            kind: SearchSourceKind::TextFile,
            label: label.into(),
            entries,
        }
    }

    fn materialize_entry(
        &self,
        entry: &ProjectSearchEntry,
        source_entry_index: usize,
    ) -> ProjectSearchEntry {
        let mut entry = entry.clone();
        entry.origin = match self.kind {
            SearchSourceKind::ProjectFile => SearchOrigin::Project {
                entry_number: match entry.origin {
                    SearchOrigin::Project { entry_number, .. } if entry_number > 0 => entry_number,
                    _ => source_entry_index + 1,
                },
                file: self.label.clone(),
            },
            SearchSourceKind::ExternalTranslationMemory => SearchOrigin::TranslationMemory {
                preamble: self.label.clone(),
            },
            SearchSourceKind::Glossary => SearchOrigin::Glossary {
                preamble: self.label.clone(),
            },
            SearchSourceKind::TextFile => SearchOrigin::Text {
                preamble: self.label.clone(),
            },
        };
        entry
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchCancellation {
    cancelled: Arc<AtomicBool>,
}

impl SearchCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchProgress {
    pub sources_total: usize,
    pub sources_visited: usize,
    pub entries_visited: usize,
    pub results_found: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchRunOutcome {
    pub completed: bool,
    pub cancelled: bool,
    pub sources_visited: usize,
    pub entries_visited: usize,
    pub results_found: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSearchEntry {
    pub source: String,
    pub translation: Option<String>,
    pub note: Option<String>,
    pub properties: Vec<(String, String)>,
    pub id: Option<String>,
    pub path: Option<String>,
    pub creator: Option<String>,
    pub changer: Option<String>,
    pub change_date: Option<i64>,
    pub origin: SearchOrigin,
}

impl ProjectSearchEntry {
    pub fn project(
        entry_number: usize,
        file: &str,
        source: &str,
        translation: Option<&str>,
    ) -> Self {
        Self {
            source: source.into(),
            translation: translation.map(str::to_string),
            note: None,
            properties: Vec::new(),
            id: None,
            path: None,
            creator: None,
            changer: None,
            change_date: None,
            origin: SearchOrigin::Project {
                entry_number,
                file: file.into(),
            },
        }
    }

    pub fn orphan(source: &str, translation: Option<&str>) -> Self {
        Self {
            origin: SearchOrigin::Orphan {
                preamble: "Orphan segment".into(),
            },
            ..Self::project(0, "", source, translation)
        }
    }

    fn property_values(&self) -> Vec<&str> {
        let mut values: Vec<&str> = self
            .properties
            .iter()
            .map(|(_, value)| value.as_str())
            .collect();
        if let SearchOrigin::Project { file, .. } = &self.origin {
            if !file.is_empty() {
                values.push(file);
            }
        }
        if let Some(id) = self.id.as_deref().filter(|s| !s.is_empty()) {
            values.push(id);
        }
        if let Some(path) = self.path.as_deref().filter(|s| !s.is_empty()) {
            values.push(path);
        }
        values
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub entry_number: i32,
    pub preamble: Option<String>,
    pub source: String,
    pub translation: Option<String>,
    pub note: Option<String>,
    pub property: Option<String>,
    pub source_matches: Vec<SearchMatch>,
    pub target_matches: Vec<SearchMatch>,
    pub note_matches: Vec<SearchMatch>,
    pub property_matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchNotCompleted;

impl std::fmt::Display for SearchNotCompleted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("search not completed yet")
    }
}

impl std::error::Error for SearchNotCompleted {}

/// Stateful, single-owner project search. It mirrors Java's non-reentrant
/// `Searcher`: results are unavailable until `run` completes, and every rerun
/// replaces (rather than appends to) the previous snapshot.
#[derive(Debug, Clone)]
pub struct Searcher {
    expression: SearchExpression,
    entries: Vec<ProjectSearchEntry>,
    sources: Vec<SearchSource>,
    results: Vec<SearchResult>,
    found_matches: Vec<SearchMatch>,
    completed: bool,
    target_locale: String,
}

impl Searcher {
    pub fn new(expression: SearchExpression) -> Self {
        Self {
            expression,
            entries: Vec::new(),
            sources: Vec::new(),
            results: Vec::new(),
            found_matches: Vec::new(),
            completed: false,
            target_locale: "und".into(),
        }
    }

    pub fn with_entries(expression: SearchExpression, entries: Vec<ProjectSearchEntry>) -> Self {
        let mut searcher = Self::new(expression);
        searcher.entries = entries;
        searcher
    }

    pub fn with_sources(expression: SearchExpression, sources: Vec<SearchSource>) -> Self {
        let mut searcher = Self::new(expression);
        searcher.sources = sources;
        searcher
    }

    pub fn get_expression(&self) -> &SearchExpression {
        &self.expression
    }

    pub fn set_target_locale(&mut self, locale: impl Into<String>) {
        self.target_locale = locale.into();
    }

    pub fn set_entries(&mut self, entries: Vec<ProjectSearchEntry>) {
        self.entries = entries;
        self.sources.clear();
        self.completed = false;
    }

    pub fn entries_mut(&mut self) -> &mut Vec<ProjectSearchEntry> {
        self.sources.clear();
        self.completed = false;
        &mut self.entries
    }

    pub fn set_sources(&mut self, sources: Vec<SearchSource>) {
        self.sources = sources;
        self.entries.clear();
        self.completed = false;
    }

    pub fn sources_mut(&mut self) -> &mut Vec<SearchSource> {
        self.entries.clear();
        self.completed = false;
        &mut self.sources
    }

    pub fn is_search_completed(&self) -> bool {
        self.completed
    }

    pub fn get_search_results(&self) -> Result<&[SearchResult], SearchNotCompleted> {
        self.completed
            .then_some(self.results.as_slice())
            .ok_or(SearchNotCompleted)
    }

    pub fn get_found_matches(&self) -> Result<&[SearchMatch], SearchNotCompleted> {
        self.completed
            .then_some(self.found_matches.as_slice())
            .ok_or(SearchNotCompleted)
    }

    pub fn get_partial_results(&self) -> &[SearchResult] {
        &self.results
    }

    pub fn search_string(&mut self, text: Option<&str>, collapse_results: bool) -> bool {
        self.found_matches = find_matches(
            text,
            &self.expression,
            collapse_results,
            &self.target_locale,
        );
        !self.found_matches.is_empty()
    }

    pub fn run(&mut self) {
        let cancellation = SearchCancellation::default();
        self.run_cancellable(&cancellation, |_| {});
    }

    pub fn run_cancellable(
        &mut self,
        cancellation: &SearchCancellation,
        mut on_progress: impl FnMut(SearchProgress),
    ) -> SearchRunOutcome {
        self.completed = false;
        self.results.clear();
        self.found_matches.clear();
        let mut deduplicated = std::collections::HashMap::new();
        let has_structured_sources = !self.sources.is_empty();
        let sources = if has_structured_sources {
            self.sources.clone()
        } else {
            vec![SearchSource::project_file("", self.entries.clone())]
        };
        let sources_total = sources.len();
        let mut sources_visited = 0;
        let mut entries_visited = 0;
        let mut cancelled = cancellation.is_cancelled();

        'sources: for source in &sources {
            if cancellation.is_cancelled() {
                cancelled = true;
                break;
            }
            sources_visited += 1;
            for (source_entry_index, raw_entry) in source.entries.iter().enumerate() {
                if cancellation.is_cancelled() {
                    cancelled = true;
                    break 'sources;
                }
                if self.results.len() >= self.expression.number_of_results {
                    break 'sources;
                }
                let entry = if has_structured_sources {
                    source.materialize_entry(raw_entry, source_entry_index)
                } else {
                    raw_entry.clone()
                };
                entries_visited += 1;
                self.consider_entry(&entry, &mut deduplicated);
                on_progress(SearchProgress {
                    sources_total,
                    sources_visited,
                    entries_visited,
                    results_found: self.results.len(),
                });
            }
        }
        cancelled |= cancellation.is_cancelled();
        self.completed = !cancelled;
        SearchRunOutcome {
            completed: self.completed,
            cancelled,
            sources_visited,
            entries_visited,
            results_found: self.results.len(),
        }
    }

    fn consider_entry(
        &mut self,
        entry: &ProjectSearchEntry,
        deduplicated: &mut std::collections::HashMap<(u8, String, String), usize>,
    ) {
        if matches!(&entry.origin, SearchOrigin::Orphan { .. }) && self.expression.exclude_orphans {
            return;
        }
        if entry.translation.is_some() && !self.expression.search_translated {
            return;
        }
        if entry.translation.is_none() && !self.expression.search_untranslated {
            return;
        }
        let Some(result) = self.match_entry(entry) else {
            return;
        };
        let duplicate_kind = match &entry.origin {
            SearchOrigin::Project { .. } => Some(0),
            SearchOrigin::TranslationMemory { .. } => Some(1),
            _ => None,
        };
        if !self.expression.all_results {
            if let Some(kind) = duplicate_kind {
                let key = (
                    kind,
                    result.source.clone(),
                    result.translation.clone().unwrap_or_default(),
                );
                if let Some(index) = deduplicated.get(&key).copied() {
                    let prior = &mut self.results[index];
                    let original = prior.preamble.clone().unwrap_or_default();
                    let (base, count) = parse_more_preamble(&original);
                    prior.preamble = Some(if base.is_empty() {
                        format!("{}\u{00a0}matches", count + 2)
                    } else {
                        format!("{base} +{}\u{00a0}more", count + 1)
                    });
                    return;
                }
                deduplicated.insert(key, self.results.len());
            }
        }
        self.results.push(result);
    }

    fn match_entry(&self, entry: &ProjectSearchEntry) -> Option<SearchResult> {
        if self.expression.search_author && !self.matches_author(entry) {
            return None;
        }
        if self.expression.date_before.is_some_and(|limit| {
            entry
                .change_date
                .is_none_or(|date| date == 0 || date >= limit)
        }) {
            return None;
        }
        if self.expression.date_after.is_some_and(|limit| {
            entry
                .change_date
                .is_none_or(|date| date == 0 || date <= limit)
        }) {
            return None;
        }

        let mut source_matches = Vec::new();
        let mut target_matches = Vec::new();
        let mut note_matches = Vec::new();
        let mut property_matches = Vec::new();
        let mut property = None;

        match self.expression.mode {
            SearchMode::Search => {
                if self.expression.search_source {
                    source_matches = find_matches(
                        Some(&entry.source),
                        &self.expression,
                        true,
                        &self.target_locale,
                    );
                }
                if self.expression.search_target {
                    target_matches = find_matches(
                        entry.translation.as_deref(),
                        &self.expression,
                        true,
                        &self.target_locale,
                    );
                }
                if self.expression.search_notes {
                    note_matches = find_matches(
                        entry.note.as_deref(),
                        &self.expression,
                        true,
                        &self.target_locale,
                    );
                }
                if self.expression.search_comments {
                    for value in entry.property_values() {
                        let matches =
                            find_matches(Some(value), &self.expression, true, &self.target_locale);
                        if !matches.is_empty() {
                            property = Some(value.to_string());
                            property_matches = matches;
                            break;
                        }
                    }
                }
                // RFE#1185: a keyword can be split between source and target,
                // with a private-use separator preventing cross-boundary
                // substring accidents.
                if source_matches.is_empty()
                    && target_matches.is_empty()
                    && self.expression.search_source
                    && self.expression.search_target
                {
                    if let Some(target) = entry.translation.as_deref() {
                        let joined = format!("{}\u{e000}{target}", entry.source);
                        source_matches = find_matches(
                            Some(&joined),
                            &self.expression,
                            true,
                            &self.target_locale,
                        );
                    }
                }
            }
            SearchMode::Replace => {
                if entry.translation.is_some() && self.expression.search_translated {
                    target_matches = find_matches(
                        entry.translation.as_deref(),
                        &self.expression,
                        false,
                        &self.target_locale,
                    );
                } else if entry.translation.is_none() && self.expression.search_untranslated {
                    source_matches = find_matches(
                        Some(&entry.source),
                        &self.expression,
                        false,
                        &self.target_locale,
                    );
                }
            }
        }
        if source_matches.is_empty()
            && target_matches.is_empty()
            && note_matches.is_empty()
            && property_matches.is_empty()
        {
            return None;
        }
        let (entry_number, preamble) = origin_fields(&entry.origin, self.expression.file_names);
        Some(SearchResult {
            entry_number,
            preamble,
            source: entry.source.clone(),
            translation: entry.translation.clone(),
            note: entry.note.clone(),
            property,
            source_matches,
            target_matches,
            note_matches,
            property_matches,
        })
    }

    fn matches_author(&self, entry: &ProjectSearchEntry) -> bool {
        let want = self.expression.author.as_deref().unwrap_or("");
        if want.is_empty() {
            return entry.creator.is_none() && entry.changer.is_none();
        }
        let mut author_expr = self.expression.clone();
        author_expr.query = want.into();
        author_expr.whole_words = false;
        entry
            .changer
            .as_deref()
            .into_iter()
            .chain(entry.creator.as_deref())
            .any(|author| {
                !find_matches(Some(author), &author_expr, true, &self.target_locale).is_empty()
            })
    }
}

fn parse_more_preamble(preamble: &str) -> (&str, usize) {
    let Some((base, rest)) = preamble.rsplit_once(" +") else {
        return (preamble, 0);
    };
    let count = rest
        .strip_suffix("\u{00a0}more")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    (base, count)
}

fn origin_fields(origin: &SearchOrigin, include_file: bool) -> (i32, Option<String>) {
    match origin {
        SearchOrigin::Project { entry_number, file } => (
            *entry_number as i32,
            include_file.then(|| file.clone()).filter(|s| !s.is_empty()),
        ),
        SearchOrigin::TranslationMemory { preamble } => {
            (-1, include_file.then(|| preamble.clone()))
        }
        SearchOrigin::Orphan { preamble } => (-2, Some(preamble.clone())),
        SearchOrigin::Alternative { preamble } => (-3, Some(preamble.clone())),
        SearchOrigin::Glossary { preamble } => (-4, Some(preamble.clone())),
        SearchOrigin::Text { preamble } => (-5, include_file.then(|| preamble.clone())),
    }
}

/*
 * Legacy implementation notes kept close to the port:
 * - Java compiles one matcher per keyword and requires every matcher to hit.
 * - regular-expression whole-word mode is intentionally ignored.
 * - source/target/property/note matches are retained separately for UI marks.
 * - duplicate accounting applies only to project and external-TM origins.
 */

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
    let mut entry = ProjectSearchEntry::project(1, "", source, translation);
    entry.note = note.map(str::to_string);
    entry.creator = creator.map(str::to_string);
    entry.properties = comments
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, value)| (format!("comment{index}"), (*value).to_string()))
        .collect();
    let mut searcher = Searcher::with_entries(expr.clone(), vec![entry]);
    searcher.run();
    searcher
        .get_search_results()
        .unwrap_or_default()
        .iter()
        .map(|result| SearchHit {
            src_text: result.source.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, translation: &str, note: &str) -> Entry {
        Entry {
            file: "a.txt".into(),
            id: "1".into(),
            prev: Some(String::new()),
            next: Some(String::new()),
            path: None,
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
        let entries = vec![entry("Hello", "Bonjour", ""), entry("Goodbye", "", "todo")];
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
