//! Hunspell `.aff`/`.dic` plus Lucene-Hunspell and Morfologik resource paths.
//!
//! Affix expansion follows the Hunspell `PFX`/`SFX` records (strip/append/condition).
//! Full language-module dictionaries stay in `reference/java` and are copied into
//! `config/spell` on first use when that tree is present.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellBackend {
    Hunspell,
    Lucene,
    Morfologik,
}

#[derive(Debug, Clone)]
struct AffRule {
    strip: String,
    append: String,
    condition: String,
}

#[derive(Debug, Default)]
pub struct AffixTable {
    prefixes: HashMap<char, Vec<AffRule>>,
    suffixes: HashMap<char, Vec<AffRule>>,
}

#[derive(Debug)]
pub struct SpellChecker {
    pub backend: SpellBackend,
    pub learned: HashSet<String>,
    pub ignored: HashSet<String>,
    pub dictionary: HashSet<String>,
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self {
            backend: SpellBackend::Hunspell,
            learned: HashSet::new(),
            ignored: HashSet::new(),
            dictionary: HashSet::new(),
        }
    }
}

impl SpellChecker {
    pub fn load(project_root: &Path, config_dir: &Path) -> Self {
        Self::load_backend(project_root, config_dir, SpellBackend::Hunspell)
    }

    pub fn load_backend(project_root: &Path, config_dir: &Path, backend: SpellBackend) -> Self {
        let mut s = Self {
            backend,
            ..Self::default()
        };
        s.learned = load_wordlist(&project_root.join("omegat").join("learned_words.txt"));
        s.ignored = load_wordlist(&project_root.join("omegat").join("ignored_words.txt"));
        if s.learned.is_empty() {
            s.learned = load_wordlist(&config_dir.join("learned_words.txt"));
        }
        for dir in language_dirs(project_root, config_dir, backend) {
            load_hunspell_dir(&dir, &mut s.dictionary);
            if backend == SpellBackend::Morfologik {
                load_wordlist_into(&dir.join("pl.dict.txt"), &mut s.dictionary);
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|e| e.to_str()) == Some("txt") {
                            load_wordlist_into(&p, &mut s.dictionary);
                        }
                    }
                }
            }
        }
        s
    }

    pub fn is_correct(&self, word: &str) -> bool {
        let w = word.to_lowercase();
        if w.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        if word.chars().any(|c| c.is_ascii_punctuation()) && word.len() == 1 {
            return true;
        }
        self.ignored.contains(&w) || self.learned.contains(&w) || self.dictionary.contains(&w)
    }

    pub fn unknown_in(&self, text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphabetic())
            .filter(|w| w.len() > 2 && !self.is_correct(w))
            .map(|w| w.to_string())
            .collect()
    }

    pub fn learn(&mut self, word: &str, project_root: &Path) {
        self.learned.insert(word.to_lowercase());
        let _ = append_word(&project_root.join("omegat").join("learned_words.txt"), word);
    }

    pub fn ignore(&mut self, word: &str, project_root: &Path) {
        self.ignored.insert(word.to_lowercase());
        let _ = append_word(&project_root.join("omegat").join("ignored_words.txt"), word);
    }
}

fn language_dirs(project_root: &Path, config_dir: &Path, backend: SpellBackend) -> Vec<PathBuf> {
    let sub = match backend {
        SpellBackend::Hunspell => "hunspell",
        SpellBackend::Lucene => "lucene",
        SpellBackend::Morfologik => "morfologik",
    };
    [
        project_root.join("omegat").join("spell").join(sub),
        config_dir.join("spell").join(sub),
        PathBuf::from("fixtures/spell").join(sub),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spell").join(sub),
        PathBuf::from("resources/languages").join(sub),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/languages").join(sub),
        // Hunspell also reads the legacy flat folder (project + config).
        project_root.join("omegat").join("spell"),
        config_dir.join("spell"),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect()
}

fn load_hunspell_dir(dir: &Path, dict: &mut HashSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut affixes: HashMap<String, AffixTable> = HashMap::new();
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) == Some("aff") {
            if let Some(table) = parse_aff(&p) {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                affixes.insert(stem, table);
            }
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "dic" {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let table = affixes.get(stem);
            load_dic_file_affixed(&p, dict, table);
        } else if ext == "txt" {
            load_dic_file(&p, dict);
        }
    }
}

pub fn parse_aff(path: &Path) -> Option<AffixTable> {
    let raw = std::fs::read_to_string(path).ok()?;
    Some(parse_aff_str(&raw))
}

pub fn parse_aff_str(raw: &str) -> AffixTable {
    let mut table = AffixTable::default();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let kind = parts[0];
        if kind != "SFX" && kind != "PFX" {
            continue;
        }
        let flag = parts[1].chars().next().unwrap_or('?');
        if parts[2] == "Y" || parts[2] == "N" {
            let n: usize = parts[3].parse().unwrap_or(0);
            let mut rules = Vec::new();
            for _ in 0..n {
                let Some(body) = lines.next() else { break };
                let bp: Vec<&str> = body.split_whitespace().collect();
                if bp.len() < 4 {
                    continue;
                }
                rules.push(AffRule {
                    strip: if bp[2] == "0" { String::new() } else { bp[2].to_string() },
                    append: if bp[3] == "0" { String::new() } else { bp[3].to_string() },
                    condition: bp.get(4).copied().unwrap_or(".").to_string(),
                });
            }
            if kind == "SFX" {
                table.suffixes.insert(flag, rules);
            } else {
                table.prefixes.insert(flag, rules);
            }
        }
    }
    table
}

/// Hunspell `.dic`: first line is count, then `word/FLAGS`.
pub fn load_dic_file(path: &Path, dict: &mut HashSet<String>) {
    load_dic_file_affixed(path, dict, None);
}

pub fn load_dic_file_affixed(path: &Path, dict: &mut HashSet<String>, aff: Option<&AffixTable>) {
    let Ok(raw) = std::fs::read_to_string(path) else { return };
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if i == 0 && line.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let (word, flags) = match line.split_once('/') {
            Some((w, f)) => (w, f),
            None => (line.split(['\t', ' ']).next().unwrap_or(line), ""),
        };
        if word.is_empty() {
            continue;
        }
        expand_word(word, flags, aff, dict);
    }
}

fn expand_word(word: &str, flags: &str, aff: Option<&AffixTable>, dict: &mut HashSet<String>) {
    dict.insert(word.to_lowercase());
    let Some(aff) = aff else { return };
    for flag in flags.chars() {
        if let Some(rules) = aff.suffixes.get(&flag) {
            for rule in rules {
                if let Some(formed) = apply_suffix(word, rule) {
                    dict.insert(formed.to_lowercase());
                }
            }
        }
        if let Some(rules) = aff.prefixes.get(&flag) {
            for rule in rules {
                if let Some(formed) = apply_prefix(word, rule) {
                    dict.insert(formed.to_lowercase());
                }
            }
        }
    }
}

fn apply_suffix(word: &str, rule: &AffRule) -> Option<String> {
    if !condition_matches(word, &rule.condition, true) {
        return None;
    }
    let stem = if rule.strip.is_empty() {
        word.to_string()
    } else if word.ends_with(&rule.strip) {
        word[..word.len() - rule.strip.len()].to_string()
    } else {
        return None;
    };
    Some(format!("{}{}", stem, rule.append))
}

fn apply_prefix(word: &str, rule: &AffRule) -> Option<String> {
    if !condition_matches(word, &rule.condition, false) {
        return None;
    }
    let stem = if rule.strip.is_empty() {
        word.to_string()
    } else if word.starts_with(&rule.strip) {
        word[rule.strip.len()..].to_string()
    } else {
        return None;
    };
    Some(format!("{}{}", rule.append, stem))
}

fn condition_matches(word: &str, cond: &str, at_end: bool) -> bool {
    if cond == "." || cond.is_empty() {
        return true;
    }
    let chars: Vec<char> = word.chars().collect();
    if cond.starts_with("[^") && cond.ends_with(']') {
        let class: HashSet<char> = cond[2..cond.len() - 1].chars().collect();
        let ch = if at_end { chars.last().copied() } else { chars.first().copied() };
        return ch.is_some_and(|c| !class.contains(&c));
    }
    if cond.starts_with('[') && cond.ends_with(']') {
        let class: HashSet<char> = cond[1..cond.len() - 1].chars().collect();
        let ch = if at_end { chars.last().copied() } else { chars.first().copied() };
        return ch.is_some_and(|c| class.contains(&c));
    }
    if at_end {
        word.ends_with(cond)
    } else {
        word.starts_with(cond)
    }
}

/// Copy Hunspell files from `reference/java/language-modules` into `dest` when present.
pub fn ensure_lang(lang: &str, dest: &Path) -> bool {
    let tag = lang.replace('_', "-").to_lowercase();
    let stem = tag.split('-').next().unwrap_or(&tag);
    std::fs::create_dir_all(dest).ok();
    if dest.join(format!("{stem}.aff")).exists() && dest.join(format!("{stem}.dic")).exists() {
        return true;
    }
    let Some((aff, dic)) = reference_dict_paths(stem) else {
        return false;
    };
    if !aff.exists() || !dic.exists() {
        return false;
    }
    let _ = std::fs::copy(&aff, dest.join(format!("{stem}.aff")));
    let _ = std::fs::copy(&dic, dest.join(format!("{stem}.dic")));
    dest.join(format!("{stem}.aff")).exists()
}

fn reference_dict_paths(stem: &str) -> Option<(PathBuf, PathBuf)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/java/language-modules");
    let (aff, dic) = match stem {
        "fr" => (
            root.join("fr/src/main/resources/org/omegat/languages/fr/fr_FR.aff"),
            root.join("fr/src/main/resources/org/omegat/languages/fr/fr_FR.dic"),
        ),
        "es" => (
            root.join("es/src/main/resources/org/omegat/languages/es/es_ES.aff"),
            root.join("es/src/main/resources/org/omegat/languages/es/es_ES.dic"),
        ),
        "ca" => (
            root.join("ca/src/main/resources/org/omegat/languages/ca/ca.aff"),
            root.join("ca/src/main/resources/org/omegat/languages/ca/ca.dic"),
        ),
        "fa" => (
            root.join("fa/src/main/resources/org/omegat/languages/fa/fa.aff"),
            root.join("fa/src/main/resources/org/omegat/languages/fa/fa.dic"),
        ),
        "ga" => (
            root.join("ga/src/main/resources/org/omegat/languages/ga/ga.aff"),
            root.join("ga/src/main/resources/org/omegat/languages/ga/ga.dic"),
        ),
        "gl" => (
            root.join("gl/src/main/resources/org/omegat/languages/gl/gl.aff"),
            root.join("gl/src/main/resources/org/omegat/languages/gl/gl.dic"),
        ),
        "pt" => (
            root.join("pt/src/main/resources/org/omegat/languages/pt/hunspell/pt_PT.aff"),
            root.join("pt/src/main/resources/org/omegat/languages/pt/hunspell/pt_PT.dic"),
        ),
        "uk" => (
            root.join("uk/src/main/resources/org/omegat/languages/uk/uk.aff"),
            root.join("uk/src/main/resources/org/omegat/languages/uk/uk.dic"),
        ),
        _ => return None,
    };
    Some((aff, dic))
}

fn load_wordlist(path: &Path) -> HashSet<String> {
    let mut s = HashSet::new();
    load_wordlist_into(path, &mut s);
    s
}

fn load_wordlist_into(path: &Path, out: &mut HashSet<String>) {
    if let Ok(raw) = std::fs::read_to_string(path) {
        for line in raw.lines() {
            let w = line.trim().to_lowercase();
            if !w.is_empty() {
                out.insert(w);
            }
        }
    }
}

fn append_word(path: &Path, word: &str) -> std::io::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{word}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn affix_expands_walks_walking() {
        let aff = parse_aff_str(
            "SFX S Y 1\nSFX S 0 s .\nSFX G Y 2\nSFX G e ing e\nSFX G 0 ing [^e]\n",
        );
        let mut set = HashSet::new();
        expand_word("walk", "SG", Some(&aff), &mut set);
        assert!(set.contains("walk"));
        assert!(set.contains("walks"));
        assert!(set.contains("walking"));
        assert!(!set.contains("wlaks"));
    }

    #[test]
    fn three_backends_use_different_paths() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spell");
        let cfg = tempfile::tempdir().unwrap();
        let hun = SpellChecker::load_backend(cfg.path(), root.parent().unwrap_or(cfg.path()), SpellBackend::Hunspell);
        // Load directly from fixture dirs
        let mut hun_set = HashSet::new();
        load_hunspell_dir(&root.join("hunspell"), &mut hun_set);
        let mut luc_set = HashSet::new();
        load_hunspell_dir(&root.join("lucene"), &mut luc_set);
        let mut mor_set = HashSet::new();
        load_hunspell_dir(&root.join("morfologik"), &mut mor_set);
        assert!(hun_set.contains("colour"), "{hun_set:?}");
        assert!(hun_set.contains("walks"), "affix must form walks");
        assert!(!hun_set.contains("color"));
        assert!(luc_set.contains("color"));
        assert!(!luc_set.contains("colour"));
        assert!(mor_set.contains("kolor"));
        assert!(!mor_set.contains("colour"));
        let _ = hun;
    }

    #[test]
    fn learn_and_ignore() {
        let dir = tempdir().unwrap();
        let mut s = SpellChecker::load(dir.path(), dir.path());
        s.learn("OmegaT", dir.path());
        s.ignore("Ctrl", dir.path());
        assert!(s.is_correct("OmegaT"));
        assert!(s.is_correct("Ctrl"));
    }
}
