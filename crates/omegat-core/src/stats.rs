use crate::matching::score_pair;
use crate::source_text_entry::Entry;
use crate::tmx::TmxEntry;
use omegat_ipc::{FileStatDto, MatchBinDto, StatCountDto, StatsDto};
use std::collections::{HashMap, HashSet};

/// Java `Character.isSpaceChar`: Zs/Zl/Zp only. `\n`/`\t` **count** as
/// characters-without-spaces (`Statistics.numberOfCharactersWithoutSpaces`).
fn is_java_space_char(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\t' | '\n' | '\r' | '\u{000B}' | '\u{000C}')
}

/// Java `Statistics.numberOfCharactersWithoutSpaces`.
pub fn number_of_characters_without_spaces(s: &str) -> usize {
    s.chars()
        .filter(|c| *c != '\u{0008}' && !is_java_space_char(*c))
        .count()
}

/// Java `Statistics.numberOfCharactersWithSpaces`.
pub fn number_of_characters_with_spaces(s: &str) -> usize {
    s.chars().filter(|c| *c != '\u{0008}').count()
}

fn chars_nosp(s: &str) -> usize {
    number_of_characters_without_spaces(s)
}

fn chars_with_spaces(s: &str) -> usize {
    number_of_characters_with_spaces(s)
}

/// Java `PatternConsts.OMEGAT_TAG`: letter **and** digits required (`<x0/>`).
static OMEGAT_TAG: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| regex::Regex::new(r"</?[a-zA-Z]+[0-9]+/?>").unwrap());

/// Java `Statistics.numberOfWords` via `DefaultTokenizer.getWordBreaker`.
pub fn number_of_words(text: &str) -> usize {
    crate::tokenize::engine::word_iterator_surfaces(text)
        .into_iter()
        .filter(|s| !OMEGAT_TAG.is_match(s.text) && s.text.chars().any(|c| c.is_alphanumeric()))
        .count()
}

fn count_source(text: &str, _lang: &str) -> (usize, usize, usize) {
    (
        number_of_words(text),
        chars_nosp(text),
        chars_with_spaces(text),
    )
}

fn add_count(dst: &mut StatCountDto, words: usize, nosp: usize, chars: usize) {
    dst.segments += 1;
    dst.words += words;
    dst.characters_without_spaces += nosp;
    dst.characters += chars;
}

pub fn compute(entries: &[Entry], source_lang: &str, target_lang: &str) -> StatsDto {
    compute_with_memory(entries, &[], source_lang, target_lang)
}

/// Java `CalcMatchStatistics` / `MatchStatCounts.getRowByPercent`.
pub fn compute_with_memory(
    entries: &[Entry],
    memory: &[TmxEntry],
    source_lang: &str,
    _target_lang: &str,
) -> StatsDto {
    let mut files_set = HashSet::new();
    let mut unique_src = HashSet::new();
    let mut unique_remaining = HashSet::new();
    let mut total = StatCountDto::default();
    let mut remaining = StatCountDto::default();
    let mut unique = StatCountDto::default();
    let mut unique_rem = StatCountDto::default();
    let mut per_file: HashMap<String, FileStatDto> = HashMap::new();
    let mut seen_unique: HashSet<(String, String)> = HashSet::new();
    let mut seen_unique_rem: HashSet<(String, String)> = HashSet::new();

    let mut source_words = 0usize;
    let mut target_words = 0usize;
    let mut source_chars = 0usize;
    let mut target_chars = 0usize;
    let mut translated = 0usize;
    let mut match_exact = 0usize;
    let mut match_fuzzy = 0usize;
    let mut bins = MatchBinDto::default();

    for e in entries {
        files_set.insert(e.file.clone());
        let (w, nosp, chars) = count_source(&e.source, source_lang);
        source_words += w;
        source_chars += chars;
        target_words += number_of_words(&e.translation);
        target_chars += e.translation.chars().count();
        add_count(&mut total, w, nosp, chars);

        let fd = per_file
            .entry(e.file.clone())
            .or_insert_with(|| FileStatDto {
                filename: e.file.clone(),
                ..Default::default()
            });
        add_count(&mut fd.total, w, nosp, chars);

        let first_unique = unique_src.insert(e.source.clone());
        if first_unique {
            add_count(&mut unique, w, nosp, chars);
        }
        if seen_unique.insert((e.file.clone(), e.source.clone())) {
            add_count(&mut fd.unique, w, nosp, chars);
        }

        if e.translated() {
            translated += 1;
            if e.from_tm_exact {
                match_exact += 1;
                bins.exact += 1;
            } else {
                match_fuzzy += 1;
                add_bin(&mut bins, best_score(&e.source, memory, source_lang));
            }
        } else {
            add_count(&mut remaining, w, nosp, chars);
            add_count(&mut fd.remaining, w, nosp, chars);
            bins.none += 1;
            if unique_remaining.insert(e.source.clone()) {
                add_count(&mut unique_rem, w, nosp, chars);
            }
            if seen_unique_rem.insert((e.file.clone(), e.source.clone())) {
                add_count(&mut fd.unique_remaining, w, nosp, chars);
            }
        }
    }

    total.files = files_set.len();
    remaining.files = entries
        .iter()
        .filter(|e| !e.translated())
        .map(|e| e.file.as_str())
        .collect::<HashSet<_>>()
        .len();
    unique.files = total.files;
    unique_rem.files = remaining.files;
    for fd in per_file.values_mut() {
        fd.total.files = 1;
        fd.remaining.files = 1;
        fd.unique.files = 1;
        fd.unique_remaining.files = 1;
    }
    let mut file_stats: Vec<FileStatDto> = per_file.into_values().collect();
    file_stats.sort_by(|a, b| a.filename.cmp(&b.filename));

    StatsDto {
        files: files_set.len(),
        segments: entries.len(),
        translated,
        unique_segments: unique_src.len(),
        source_words,
        target_words,
        source_chars,
        target_chars,
        match_exact,
        match_fuzzy,
        match_none: entries.len().saturating_sub(translated),
        total,
        remaining,
        unique,
        unique_remaining: unique_rem,
        file_stats,
        match_bins: bins,
    }
}

fn best_score(source: &str, memory: &[TmxEntry], lang: &str) -> i32 {
    memory
        .iter()
        .map(|m| score_pair(source, &m.source, lang).0)
        .max()
        .unwrap_or(0)
}

/// Java `Statistics.PERCENT_EXACT_MATCH` — not a FuzzyMatcher 100 score.
pub const PERCENT_EXACT_MATCH: i32 = 101;

/// Java `MatchStatCounts` row names used by `CalcMatchStatistics`.
pub const MATCH_ROWS: [&str; 8] = [
    "repetition",
    "exact",
    "fuzzy_95",
    "fuzzy_85",
    "fuzzy_75",
    "fuzzy_50",
    "none",
    "total",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchRow {
    pub segments: i64,
    pub words: i64,
    pub chars_nosp: i64,
    pub chars: i64,
}

impl MatchRow {
    pub fn add(&mut self, words: usize, nosp: usize, chars: usize) {
        self.segments += 1;
        self.words += words as i64;
        self.chars_nosp += nosp as i64;
        self.chars += chars as i64;
    }
}

/// Java `CalcMatchStatistics.calcTotal` bins (no per-file other-file row).
pub fn calc_match_bins(
    sources: &[String],
    translated: &[bool],
    memory: &[TmxEntry],
    lang: &str,
) -> Vec<MatchRow> {
    calc_match_bins_ex(
        sources,
        translated,
        memory,
        &[],
        &[],
        crate::tokenize::tokenizer_id(lang),
        lang,
        lang,
    )
}

/// Full `CalcMatchStatistics.calcTotal` + `calcSimilarity` against extra TM
/// and `SourceTextEntry.getSourceTranslation` (FILES).
pub fn calc_match_bins_ex(
    sources: &[String],
    translated: &[bool],
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    files: &[crate::find_matches::FileTranslation],
    tokenizer: &str,
    source_lang: &str,
    target_lang: &str,
) -> Vec<MatchRow> {
    let mut rows = vec![MatchRow::default(); 8];
    let mut seen = HashSet::new();
    let mut pending = Vec::new();
    for (src, done) in sources.iter().zip(translated.iter()) {
        let (w, nosp, chars) = count_source(src, source_lang);
        if *done {
            rows[1].add(w, nosp, chars);
        } else if !seen.insert(src.clone()) {
            rows[0].add(w, nosp, chars);
        } else {
            pending.push((src.clone(), w, nosp, chars));
        }
        rows[7].add(w, nosp, chars);
    }
    for (src, w, nosp, chars) in pending {
        let best = calc_max_similarity(
            &src,
            memory,
            extra,
            files,
            tokenizer,
            source_lang,
            target_lang,
        );
        let idx = match bin_for_percent(best) {
            "exact" => 1,
            "fuzzy_95" => 2,
            "fuzzy_85" => 3,
            "fuzzy_75" => 4,
            "fuzzy_50" => 5,
            _ => 6,
        };
        rows[idx].add(w, nosp, chars);
    }
    rows
}

/// Java `CalcMatchStatistics.calcMaxSimilarity`.
pub fn calc_max_similarity(
    source: &str,
    memory: &[TmxEntry],
    extra: &[(TmxEntry, String)],
    files: &[crate::find_matches::FileTranslation],
    tokenizer: &str,
    source_lang: &str,
    target_lang: &str,
) -> i32 {
    // Java `FindMatches.search` enables paragraph/subsegment TM when
    // `PARAGRAPH_MATCH_FROM_SEGMENT_TMX` (default true) and the project is
    // not sentence-segmented. `CalcMatchStatisticsTest` uses that setup.
    let nears = crate::find_matches::search(crate::find_matches::SearchRequest {
        query: source,
        memory,
        extra,
        files,
        tokenizer,
        source_lang,
        target_lang,
        threshold: -1,
        limit: crate::consts::MAX_NEAR_STRINGS,
        search_exactly_the_same: false,
        run_separate_segment_match: true,
        foreign_penalty: crate::find_matches::PENALTY_FOR_FOREIGN_MATCHES_DEFAULT,
    });
    let src_tokens = crate::find_matches::tokenize_all(source);
    let mut max_sim = 0;
    for near in nears {
        let cand = crate::find_matches::tokenize_all(&near.source);
        let mut sim = crate::levenshtein::token_similarity(&src_tokens, &cand);
        if near.fuzzy {
            sim -= crate::find_matches::PENALTY_FOR_FUZZY;
        }
        if sim > max_sim {
            max_sim = sim;
            if new_enough_for_95(max_sim) {
                break;
            }
        }
    }
    max_sim
}

fn new_enough_for_95(sim: i32) -> bool {
    sim >= 95
}

/// Java `MatchStatCounts.getRowByPercent`.
/// Exact is only `101`. A FuzzyMatcher score of 100 lands in the 95% bin.
pub fn bin_for_percent(percent: i32) -> &'static str {
    if percent == PERCENT_EXACT_MATCH {
        "exact"
    } else if percent >= 95 {
        "fuzzy_95"
    } else if percent >= 85 {
        "fuzzy_85"
    } else if percent >= 75 {
        "fuzzy_75"
    } else if percent >= 50 {
        "fuzzy_50"
    } else {
        "none"
    }
}

fn add_bin(bins: &mut MatchBinDto, percent: i32) {
    match bin_for_percent(percent) {
        "exact" => bins.exact += 1,
        "fuzzy_95" => bins.fuzzy_95 += 1,
        "fuzzy_85" => bins.fuzzy_85 += 1,
        "fuzzy_75" => bins.fuzzy_75 += 1,
        "fuzzy_50" => bins.fuzzy_50 += 1,
        _ => bins.none += 1,
    }
}

/// Java `StatisticsTextWriter` English bundle keys.
pub fn render_text(stats: &StatsDto) -> String {
    let mut out = String::from("Project Statistics\n\n");
    out.push_str(&format!(
        "{:<22} {:>10} {:>10} {:>24} {:>22} {:>8}\n",
        "", "Segments", "Words", "Characters (w/o spaces)", "Characters (w/ spaces)", "#Files"
    ));
    for (label, c) in [
        ("Total:", &stats.total),
        ("Remaining:", &stats.remaining),
        ("Unique:", &stats.unique),
        ("Unique Remaining:", &stats.unique_remaining),
    ] {
        out.push_str(&format!(
            "{:<22} {:>10} {:>10} {:>24} {:>22} {:>8}\n",
            label, c.segments, c.words, c.characters_without_spaces, c.characters, c.files
        ));
    }
    out.push_str("\nIndividual File Statistics:\n\n");
    out.push_str("File Name\tTotal Segments\tRemaining Segments\tUnique Segments\tUnique Remaining Segments\tTotal Words\tRemaining Words\tUnique Words\tUnique Remaining Words\tTotal Characters (w/o spaces)\tRemaining Characters (w/o spaces)\tUnique Characters (w/o spaces)\tUnique Remaining Characters (w/o spaces)\tTotal Characters (w/ spaces)\tRemaining Characters (w/ spaces)\tUnique Characters (w/ spaces)\tUnique Remaining Characters (w/ spaces)\n");
    for f in &stats.file_stats {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            f.filename,
            f.total.segments,
            f.remaining.segments,
            f.unique.segments,
            f.unique_remaining.segments,
            f.total.words,
            f.remaining.words,
            f.unique.words,
            f.unique_remaining.words,
            f.total.characters_without_spaces,
            f.remaining.characters_without_spaces,
            f.unique.characters_without_spaces,
            f.unique_remaining.characters_without_spaces,
            f.total.characters,
            f.remaining.characters,
            f.unique.characters,
            f.unique_remaining.characters
        ));
    }
    out
}

pub fn render_json(stats: &StatsDto) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(stats)
}

pub fn render_xml(stats: &StatsDto) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<omegat-stats>\n");
    fn write_count(out: &mut String, name: &str, c: &StatCountDto) {
        out.push_str(&format!(
            "  <{name} segments=\"{}\" words=\"{}\" characters-without-spaces=\"{}\" characters=\"{}\" files=\"{}\"/>\n",
            c.segments, c.words, c.characters_without_spaces, c.characters, c.files
        ));
    }
    write_count(&mut out, "total", &stats.total);
    write_count(&mut out, "remaining", &stats.remaining);
    write_count(&mut out, "unique", &stats.unique);
    write_count(&mut out, "unique-remaining", &stats.unique_remaining);
    out.push_str("  <files>\n");
    for f in &stats.file_stats {
        out.push_str(&format!(
            "    <file filename=\"{}\">\n",
            xml_esc(&f.filename)
        ));
        write_count(&mut out, "total", &f.total);
        write_count(&mut out, "remaining", &f.remaining);
        write_count(&mut out, "unique", &f.unique);
        write_count(&mut out, "unique-remaining", &f.unique_remaining);
        out.push_str("    </file>\n");
    }
    out.push_str("  </files>\n</omegat-stats>\n");
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsResultMetadata {
    pub project_name: String,
    pub project_root: String,
    pub source_language: String,
    pub target_language: String,
}

/// Java `StatisticsXmlWriter` schema, with an explicit date so callers and
/// golden tests do not depend on wall-clock time.
pub fn render_stats_result_xml(
    stats: &StatsDto,
    metadata: &StatsResultMetadata,
    date: &str,
) -> String {
    fn attrs(count: &StatCountDto) -> String {
        format!(
            "segments=\"{}\" words=\"{}\" characters-without-spaces=\"{}\" characters=\"{}\" files=\"{}\"",
            count.segments,
            count.words,
            count.characters_without_spaces,
            count.characters,
            count.files
        )
    }

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<omegat-stats>\n");
    out.push_str(&format!(
        "  <project name=\"{}\" root=\"{}\" source-language=\"{}\" target-language=\"{}\"/>\n",
        xml_esc(&metadata.project_name),
        xml_esc(&metadata.project_root),
        xml_esc(&metadata.source_language),
        xml_esc(&metadata.target_language)
    ));
    out.push_str(&format!("  <total {}/>\n", attrs(&stats.total)));
    out.push_str(&format!("  <remaining {}/>\n", attrs(&stats.remaining)));
    out.push_str(&format!("  <unique {}/>\n", attrs(&stats.unique)));
    out.push_str(&format!(
        "  <unique-remaining {}/>\n",
        attrs(&stats.unique_remaining)
    ));
    out.push_str("  <files>\n");
    for file in &stats.file_stats {
        out.push_str(&format!(
            "    <filename>{}</filename>\n",
            xml_text_esc(&file.filename)
        ));
        out.push_str(&format!("    <total {}/>\n", attrs(&file.total)));
        out.push_str(&format!("    <unique {}/>\n", attrs(&file.unique)));
        out.push_str(&format!("    <remaining {}/>\n", attrs(&file.remaining)));
        out.push_str(&format!(
            "    <unique-remaining {}/>\n",
            attrs(&file.unique_remaining)
        ));
    }
    out.push_str(&format!(
        "  </files>\n  <date>{}</date>\n</omegat-stats>\n",
        xml_text_esc(date)
    ));
    out
}

pub fn render(stats: &StatsDto, kind: &str) -> String {
    match kind {
        "json" => render_json(stats).unwrap_or_else(|_| "{}".into()),
        "xml" => render_xml(stats),
        _ => render_text(stats),
    }
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

fn xml_text_esc(s: &str) -> String {
    xml_esc(s).replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_text_entry::Entry;

    fn entry(file: &str, src: &str, tgt: &str) -> Entry {
        Entry {
            file: file.into(),
            id: src.into(),
            prev: Some(String::new()),
            next: Some(String::new()),
            path: None,
            source: src.into(),
            translation: tgt.into(),
            note: String::new(),
            comment: String::new(),
            default_translation: true,
            revision: 1,
            from_tm_exact: !tgt.is_empty(),
            properties: vec![],
        }
    }

    #[test]
    fn statistics_test_number_of_words() {
        // Java `StatisticsTest#testNumberOfWords`
        assert_eq!(number_of_words("one two three"), 3);
        assert_eq!(number_of_words("one , \u{8} two three"), 3);
        assert_eq!(number_of_words("o\u{8}ne <b>two</b>"), 5);
        // UAX #29 keeps hyphenated AHLetter tokens together (header of the PO fixture).
        assert_eq!(number_of_words("Content-Type"), 1);
        assert_eq!(number_of_words("X-Language"), 1);
        assert_eq!(number_of_words("can't have emoji"), 3);
        assert_eq!(number_of_words("doesn't match"), 2);
    }

    #[test]
    fn java_shaped_counts_and_formats() {
        let entries = vec![
            entry("a.txt", "Hello world", "Bonjour le monde"),
            entry("a.txt", "Second", ""),
            entry("b.txt", "Hello world", ""),
        ];
        let s = compute(&entries, "en", "fr");
        assert_eq!(s.total.segments, 3);
        assert_eq!(s.remaining.segments, 2);
        assert_eq!(s.unique.segments, 2);
        assert_eq!(s.unique_remaining.segments, 2);
        assert_eq!(s.file_stats.len(), 2);
        let text = render_text(&s);
        assert!(text.contains("Project Statistics"));
        assert!(text.contains("Total:"));
        assert!(text.contains("Individual File Statistics:"));
        let json = render_json(&s).unwrap();
        assert!(json.contains("\"unique-remaining\""));
        let xml = render_xml(&s);
        assert!(xml.contains("<omegat-stats>"));
        assert!(xml.contains("characters-without-spaces"));
    }
}
