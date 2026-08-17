use crate::tokenize::word_count;
use omegat_ipc::StatsDto;

use crate::session::Entry;

pub fn compute(entries: &[Entry], source_lang: &str, target_lang: &str) -> StatsDto {
    let files = entries
        .iter()
        .map(|e| e.file.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let translated = entries.iter().filter(|e| e.translated()).count();
    let unique = entries
        .iter()
        .map(|e| e.source.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let source_words = entries
        .iter()
        .map(|e| word_count(&e.source, source_lang))
        .sum();
    let target_words = entries
        .iter()
        .map(|e| word_count(&e.translation, target_lang))
        .sum();
    let source_chars = entries.iter().map(|e| e.source.chars().count()).sum();
    let target_chars = entries.iter().map(|e| e.translation.chars().count()).sum();
    let match_exact = entries.iter().filter(|e| e.from_tm_exact).count();
    let match_fuzzy = entries
        .iter()
        .filter(|e| e.translated() && !e.from_tm_exact)
        .count();
    StatsDto {
        files,
        segments: entries.len(),
        translated,
        unique_segments: unique,
        source_words,
        target_words,
        source_chars,
        target_chars,
        match_exact,
        match_fuzzy,
        match_none: entries.len().saturating_sub(translated),
    }
}
