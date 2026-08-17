use crate::session::Entry;
use crate::tokenize::word_count;
use omegat_ipc::{FileStatDto, MatchBinDto, StatCountDto, StatsDto};
use std::collections::{HashMap, HashSet};

fn chars_nosp(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

fn count_source(text: &str, lang: &str) -> (usize, usize, usize) {
    (
        word_count(text, lang),
        chars_nosp(text),
        text.chars().count(),
    )
}

fn add_count(dst: &mut StatCountDto, words: usize, nosp: usize, chars: usize) {
    dst.segments += 1;
    dst.words += words;
    dst.characters_without_spaces += nosp;
    dst.characters += chars;
}

pub fn compute(entries: &[Entry], source_lang: &str, target_lang: &str) -> StatsDto {
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
        target_words += word_count(&e.translation, target_lang);
        target_chars += e.translation.chars().count();
        add_count(&mut total, w, nosp, chars);

        let fd = per_file.entry(e.file.clone()).or_insert_with(|| FileStatDto {
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
                bins.fuzzy_85 += 1;
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
        out.push_str(&format!("    <file filename=\"{}\">\n", xml_esc(&f.filename)));
        write_count(&mut out, "total", &f.total);
        write_count(&mut out, "remaining", &f.remaining);
        write_count(&mut out, "unique", &f.unique);
        write_count(&mut out, "unique-remaining", &f.unique_remaining);
        out.push_str("    </file>\n");
    }
    out.push_str("  </files>\n</omegat-stats>\n");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Entry;

    fn entry(file: &str, src: &str, tgt: &str) -> Entry {
        Entry {
            file: file.into(),
            id: src.into(),
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
