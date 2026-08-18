//! Java `org.omegat.gui.glossary.GlossarySearcher` plus TSV/TBX readers.

use crate::language::Language;
use crate::string_util::{is_cjk, is_upper_case, is_white_space_cp};
use crate::tokenize::{tokenize_word_tokens, StemmingMode};
use omegat_ipc::GlossaryHitDto;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
    pub comment: String,
    pub priority: bool,
    pub loc_terms: Vec<String>,
    pub comments: Vec<String>,
    pub priorities: Vec<bool>,
}

impl PartialEq for GlossaryEntry {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.target == other.target && self.comment == other.comment
    }
}

impl GlossaryEntry {
    pub fn new(source: &str, target: &str, comment: &str) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            comment: comment.into(),
            priority: false,
            loc_terms: if target.is_empty() {
                vec![]
            } else {
                vec![target.into()]
            },
            comments: if comment.is_empty() {
                vec![]
            } else {
                vec![comment.into()]
            },
            priorities: vec![false],
        }
    }

    pub fn with_priority(mut self, priority: bool) -> Self {
        self.priority = priority;
        if let Some(p) = self.priorities.first_mut() {
            *p = priority;
        }
        self
    }

    pub fn loc_terms(&self) -> Vec<String> {
        if !self.loc_terms.is_empty() {
            return self.loc_terms.clone();
        }
        if self.target.is_empty() {
            vec![]
        } else {
            vec![self.target.clone()]
        }
    }

    /// Java `DefaultGlossaryRenderer.renderToHtml`.
    pub fn render_to_html(&self) -> String {
        let mut locs = String::new();
        let terms = if self.loc_terms.is_empty() {
            vec![self.target.clone()]
        } else {
            self.loc_terms.clone()
        };
        for (i, t) in terms.iter().enumerate() {
            if i > 0 {
                locs.push_str(", ");
            }
            let pri = *self.priorities.get(i).unwrap_or(&self.priority);
            if pri {
                locs.push_str(&format!("<b>{t}</b>"));
            } else {
                locs.push_str(t);
            }
        }
        let mut comments = String::new();
        let comms = if self.comments.is_empty() {
            if self.comment.is_empty() {
                vec![]
            } else {
                vec![self.comment.clone()]
            }
        } else {
            self.comments.clone()
        };
        for (i, c) in comms.iter().enumerate() {
            if c.is_empty() {
                continue;
            }
            comments.push_str(&format!("<br>{}. {c}", i + 1));
        }
        format!("<html><p>{} = {locs}{comments}</p></html>", self.source)
    }
}

/// Java `org.omegat.gui.glossary.TransTipsMarker.Mark`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TipMark {
    pub start: usize,
    pub end: usize,
    pub tooltip: String,
}

/// Java `TransTipsMarker.getMarksForEntry`.
/// `None` = Java `null` (inactive / null source / marking off / no entries).
/// `Some(vec![])` = empty token matches.
pub fn marks_for_entry(
    source: Option<&str>,
    entries: &[GlossaryEntry],
    active: bool,
    mark_glossary: bool,
) -> Option<Vec<TipMark>> {
    if !active {
        return None;
    }
    let src = source?;
    if !mark_glossary {
        return None;
    }
    if entries.is_empty() {
        return None;
    }
    let mut marks = Vec::new();
    let lower = src.to_lowercase();
    for e in entries {
        if e.source.is_empty() {
            continue;
        }
        let needle = e.source.to_lowercase();
        if let Some(pos) = lower.find(&needle) {
            marks.push(TipMark {
                start: pos,
                end: pos + e.source.len(),
                tooltip: e.render_to_html(),
            });
        }
    }
    Some(marks)
}

#[derive(Debug, Clone)]
pub struct GlossarySearcher {
    pub src_lang: Language,
    pub target_lang: Language,
    pub tokenizer_class: String,
    pub merge_alt_definitions: bool,
    pub stemming: bool,
    pub stemming_full: bool,
    pub not_exact_match: bool,
    pub require_similar_case: bool,
    pub sort_by_src_length: bool,
    pub sort_by_length: bool,
}

impl GlossarySearcher {
    pub fn new(src_lang: &str, target_lang: &str, tokenizer_class: &str) -> Self {
        Self {
            src_lang: Language::new(Some(src_lang)),
            target_lang: Language::new(Some(target_lang)),
            tokenizer_class: tokenizer_class.into(),
            merge_alt_definitions: true,
            stemming: true,
            stemming_full: false,
            not_exact_match: false,
            require_similar_case: true,
            sort_by_src_length: true,
            sort_by_length: false,
        }
    }

    pub fn search_source_matches(&self, source: &str, entries: &[GlossaryEntry]) -> Vec<GlossaryEntry> {
        let tags = tag_spans(source);
        let tokens = self.tokenize_skipping_tags(source, &tags);
        let mut result = Vec::new();
        for e in entries {
            if self.is_token_match(&tokens, source, &e.source) || self.is_cjk_match(source, &e.source) {
                result.push(e.clone());
            }
        }
        self.sort_and_filter(result)
    }

    pub fn search_target_matches(&self, trg: &str, entry: &GlossaryEntry) -> Vec<String> {
        let tags = tag_spans(trg);
        let tokens = self.tokenize_skipping_tags(trg, &tags);
        let mut result = Vec::new();
        for term in entry.loc_terms() {
            if self.is_token_match(&tokens, trg, &term) || self.is_cjk_match(trg, &term) {
                result.push(term);
            }
        }
        result
    }

    pub fn tokenize(&self, str: &str) -> Vec<String> {
        if self.stemming {
            let mode = if self.stemming_full {
                StemmingMode::GlossaryFull
            } else {
                StemmingMode::Glossary
            };
            tokenize_word_tokens(&str.to_lowercase(), &self.tokenizer_class, mode)
                .into_iter()
                .filter(|t| !t.chars().all(is_white_space_cp))
                .collect()
        } else {
            tokenize_verbatim_non_ws(str)
        }
    }

    pub fn search_source_match_tokens(&self, source: &str, term: &str) -> Vec<Vec<String>> {
        let tags = tag_spans(source);
        let tokens = self.tokenize_skipping_tags(source, &tags);
        let found = self.matching_tokens(&tokens, source, term);
        if found.is_empty() && is_cjk(term) && source.contains(term) {
            return vec![vec![term.to_string()]];
        }
        found
    }

    pub fn sort_glossary_entries(&self, mut entries: Vec<GlossaryEntry>) -> Vec<GlossaryEntry> {
        entries.sort_by(|a, b| self.compare_entries(a, b));
        entries
    }

    fn compare_entries(&self, o1: &GlossaryEntry, o2: &GlossaryEntry) -> std::cmp::Ordering {
        let p1 = if o1.priority { 1 } else { 2 };
        let p2 = if o2.priority { 1 } else { 2 };
        let mut c = p1.cmp(&p2);
        if c == std::cmp::Ordering::Equal
            && self.sort_by_src_length
            && (o2.source.starts_with(&o1.source) || o1.source.starts_with(&o2.source))
        {
            c = o2.source.len().cmp(&o1.source.len());
        }
        if c == std::cmp::Ordering::Equal {
            c = ja_or_latin_cmp(&o1.source, &o2.source, &self.src_lang);
        }
        if c == std::cmp::Ordering::Equal && self.sort_by_length {
            c = o2.target.len().cmp(&o1.target.len());
        }
        if c == std::cmp::Ordering::Equal {
            c = ja_or_latin_cmp(&o1.target, &o2.target, &self.target_lang);
        }
        c
    }

    fn tokenize_skipping_tags(&self, str: &str, tags: &[(usize, String)]) -> Vec<(String, usize)> {
        let toks = self.tokenize_with_offsets(str);
        toks.into_iter()
            .filter(|(tok, off)| {
                !tags.iter().any(|(pos, tag)| *off >= *pos && *off + tok.len() <= pos + tag.len())
            })
            .collect()
    }

    fn tokenize_with_offsets(&self, str: &str) -> Vec<(String, usize)> {
        let words = self.tokenize(str);
        let lower = str.to_lowercase();
        let mut out = Vec::new();
        let mut from = 0;
        for w in words {
            let needle = w.to_lowercase();
            if let Some(abs) = find_from(&lower, from, &needle) {
                out.push((w.clone(), abs));
                from = (abs + w.len()).min(lower.len());
                while from < lower.len() && !lower.is_char_boundary(from) {
                    from += 1;
                }
            } else if let Some(abs) = lower.find(&w) {
                out.push((w, abs));
            } else {
                out.push((w, from));
            }
        }
        out
    }

    fn is_token_match(&self, full: &[(String, usize)], full_text: &str, term: &str) -> bool {
        !self.matching_tokens(full, full_text, term).is_empty()
    }

    fn matching_tokens(&self, full: &[(String, usize)], full_text: &str, term: &str) -> Vec<Vec<String>> {
        let glos = self.tokenize(term);
        if glos.is_empty() {
            return vec![];
        }
        let mut found = search_all(full, &glos, self.not_exact_match);
        found.retain(|toks| self.keep_match(toks, full_text, term));
        if is_cjk(term) {
            found.retain(|toks| toks.iter().any(|t| term.contains(t)));
        }
        found
    }

    fn keep_match(&self, tokens: &[String], src_txt: &str, loc_txt: &str) -> bool {
        if self.require_similar_case && is_upper_case(loc_txt) {
            for tok in tokens {
                if let Some(idx) = src_txt.to_lowercase().find(&tok.to_lowercase()) {
                    let matched = src_txt.get(idx..idx + tok.len()).unwrap_or(tok);
                    if !is_upper_case(matched) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn is_cjk_match(&self, full_text: &str, term: &str) -> bool {
        is_cjk_match(full_text, term)
    }

    fn sort_and_filter(&self, result: Vec<GlossaryEntry>) -> Vec<GlossaryEntry> {
        let result = self.sort_glossary_entries(result);
        if !self.merge_alt_definitions {
            return result;
        }
        let mut merged: Vec<GlossaryEntry> = Vec::new();
        for e in result {
            if let Some(prev) = merged.iter_mut().find(|p| p.source.eq_ignore_ascii_case(&e.source)) {
                if !prev.target.split(" / ").any(|t| t == e.target) {
                    if !prev.target.is_empty() && !e.target.is_empty() {
                        prev.target = format!("{} / {}", prev.target, e.target);
                    } else if prev.target.is_empty() {
                        prev.target = e.target;
                    }
                }
            } else {
                merged.push(e);
            }
        }
        merged
    }
}

/// Java `GlossarySearcher.isCjkMatch` once a CJK project is loaded.
pub fn is_cjk_match(full_text: &str, term: &str) -> bool {
    is_cjk(term) && full_text.contains(term)
}

fn find_from(hay: &str, from: usize, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(hay.len()));
    }
    let start = if from <= hay.len() && hay.is_char_boundary(from) {
        from
    } else {
        0
    };
    hay.get(start..)?.find(needle).map(|i| start + i)
}

fn tag_spans(text: &str) -> Vec<(usize, String)> {
    let re = regex::Regex::new(r"<[^>]+>|\{[0-9]+\}").unwrap();
    re.find_iter(text).map(|m| (m.start(), m.as_str().to_string())).collect()
}

fn tokenize_verbatim_non_ws(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else if ch.is_alphanumeric() {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn search_all(full: &[(String, usize)], glos: &[String], not_exact: bool) -> Vec<Vec<String>> {
    let mut hits = Vec::new();
    if glos.is_empty() {
        return hits;
    }
    for i in 0..full.len() {
        if token_eq(&full[i].0, &glos[0], not_exact) {
            if glos.len() == 1 {
                hits.push(vec![full[i].0.clone()]);
                continue;
            }
            let mut ok = true;
            let mut matched = vec![full[i].0.clone()];
            for (k, g) in glos.iter().enumerate().skip(1) {
                let j = i + k;
                if j >= full.len() || !token_eq(&full[j].0, g, not_exact) {
                    ok = false;
                    break;
                }
                matched.push(full[j].0.clone());
            }
            if ok {
                hits.push(matched);
            }
        }
    }
    hits
}

fn token_eq(a: &str, b: &str, not_exact: bool) -> bool {
    let al = a.to_lowercase();
    let bl = b.to_lowercase();
    if al == bl {
        return true;
    }
    if not_exact {
        al.starts_with(&bl) || bl.starts_with(&al)
    } else {
        false
    }
}

fn ja_or_latin_cmp(a: &str, b: &str, lang: &Language) -> std::cmp::Ordering {
    if lang.get_language_code() == "ja" {
        ja_rank(a).cmp(&ja_rank(b)).then_with(|| a.cmp(b))
    } else {
        a.to_lowercase().cmp(&b.to_lowercase())
    }
}

fn ja_rank(s: &str) -> i32 {
    match s.chars().next() {
        Some(c) if ('\u{3040}'..='\u{309F}').contains(&c) => 0,
        Some(c) if ('\u{30A0}'..='\u{30FF}').contains(&c) => 1,
        Some('向') => 2,
        Some('上') => 3,
        Some(c) if (c as u32) >= 0x4E00 => 4,
        _ => 5,
    }
}

pub fn load_glossary(path: &Path) -> Vec<GlossaryEntry> {
    if !path.exists() {
        return vec![];
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vec![];
    };
    parse_glossary(&raw)
}

pub fn parse_glossary(raw: &str) -> Vec<GlossaryEntry> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('<') && line.contains("<term") {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            out.push(GlossaryEntry::new(parts[0].trim(), parts[1].trim(), parts.get(2).unwrap_or(&"").trim()));
        }
    }
    if out.is_empty() && raw.contains("<term") {
        let mut terms = Vec::new();
        let mut rest = raw;
        while let Some(s) = rest.find("<term") {
            let after = &rest[s..];
            if let Some(gt) = after.find('>') {
                if let Some(end) = after[gt + 1..].find("</term>") {
                    terms.push(after[gt + 1..gt + 1 + end].to_string());
                    rest = &after[gt + 1 + end + 7..];
                    continue;
                }
            }
            break;
        }
        for pair in terms.chunks(2) {
            if pair.len() == 2 {
                out.push(GlossaryEntry::new(&pair[0], &pair[1], "tbx"));
            }
        }
    }
    out
}

pub fn lookup(entries: &[GlossaryEntry], segment: &str) -> Vec<GlossaryHitDto> {
    lookup_opts(entries, segment, true, true)
}

pub fn lookup_opts(entries: &[GlossaryEntry], segment: &str, ignore_case: bool, use_stem: bool) -> Vec<GlossaryHitDto> {
    lookup_opts_lang(entries, segment, ignore_case, use_stem, "en")
}

pub fn lookup_opts_lang(
    entries: &[GlossaryEntry],
    segment: &str,
    _ignore_case: bool,
    use_stem: bool,
    stem_lang: &str,
) -> Vec<GlossaryHitDto> {
    let tok = match stem_lang {
        "it" | "ita" => "org.omegat.tokenizer.LuceneItalianTokenizer",
        "ja" | "jpn" => "org.omegat.tokenizer.LuceneJapaneseTokenizer",
        "ko" | "kor" => "org.omegat.tokenizer.LuceneCJKTokenizer",
        "zh" | "zho" => "org.omegat.tokenizer.LuceneSmartChineseTokenizer",
        _ => "org.omegat.tokenizer.LuceneEnglishTokenizer",
    };
    let mut searcher = GlossarySearcher::new("en", stem_lang, tok);
    searcher.stemming = use_stem;
    searcher
        .search_source_matches(segment, entries)
        .into_iter()
        .map(|e| GlossaryHitDto {
            source: e.source,
            target: e.target,
            comment: e.comment,
        })
        .collect()
}

pub fn append_entry(path: &Path, source: &str, target: &str, comment: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = format!("{source}\t{target}");
    if !comment.is_empty() {
        line.push('\t');
        line.push_str(comment);
    }
    line.push('\n');
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_and_stem_lookup() {
        let entries = parse_glossary("running\tcourir\tverb\n");
        let hits = lookup_opts(&entries, "The runner is running", true, true);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].target, "courir");
    }

    #[test]
    fn tbx_pairs() {
        let raw = r#"<martif><term>cat</term><term>chat</term></martif>"#;
        let entries = parse_glossary(raw);
        assert_eq!(entries[0].source, "cat");
        assert_eq!(entries[0].target, "chat");
    }

    #[test]
    fn english_exact_source_match() {
        let searcher = GlossarySearcher::new("en", "fr", "org.omegat.tokenizer.LuceneEnglishTokenizer");
        let entries = vec![GlossaryEntry::new("dog", "chien", "")];
        let hits = searcher.search_source_matches("The dog barked", &entries);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, "chien");
    }

    #[test]
    fn cjk_contains_when_source_not_space_delimited() {
        let searcher = GlossarySearcher::new("ja", "en", "org.omegat.tokenizer.LuceneJapaneseTokenizer");
        let entries = vec![GlossaryEntry::new("日本語", "Japanese", "")];
        let hits = searcher.search_source_matches("これは日本語です", &entries);
        assert_eq!(hits.len(), 1);
    }
}
