//! Hunspell `.aff`/`.dic` plus Lucene-Hunspell and Morfologik resource paths.
//!
//! Affix expansion follows the Hunspell `PFX`/`SFX` records (strip/append/condition).
//! Full language-module dictionaries stay in `reference/java` and are copied into
//! `config/spell` on first use when that tree is present.

use omegat_ipc::SpellTokenDto;
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static INSTALL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellBackend {
    Hunspell,
    Lucene,
    Morfologik,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FlagKind {
    #[default]
    Char,
    Long,
    Num,
}

#[derive(Debug, Clone)]
struct AffRule {
    flag: String,
    strip: String,
    append: String,
    condition: String,
}

#[derive(Debug, Clone, Default)]
pub struct AffixTable {
    kind: FlagKind,
    prefixes: HashMap<String, Vec<AffRule>>,
    suffixes: HashMap<String, Vec<AffRule>>,
}

#[derive(Debug, Clone)]
pub struct SpellChecker {
    pub backend: SpellBackend,
    pub learned: HashSet<String>,
    pub ignored: HashSet<String>,
    pub dictionary: HashSet<String>,
    stems: HashMap<String, String>,
    affix: AffixTable,
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self {
            backend: SpellBackend::Hunspell,
            learned: HashSet::new(),
            ignored: HashSet::new(),
            dictionary: HashSet::new(),
            stems: HashMap::new(),
            affix: AffixTable::default(),
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
            load_hunspell_dir(&dir, &mut s.dictionary, &mut s.stems, &mut s.affix);
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
        self.ignored.contains(&w)
            || self.learned.contains(&w)
            || self.dictionary.contains(&w)
            || self.stems.contains_key(&w)
            || self.formed_by_affix(&w)
    }

    fn formed_by_affix(&self, word: &str) -> bool {
        for rules in self.affix.suffixes.values() {
            for rule in rules {
                if let Some(stem) = unapply_suffix(word, rule) {
                    if flags_contain(self.stems.get(&stem).map(String::as_str).unwrap_or(""), &rule.flag, self.affix.kind)
                    {
                        return true;
                    }
                }
            }
        }
        for rules in self.affix.prefixes.values() {
            for rule in rules {
                if let Some(stem) = unapply_prefix(word, rule) {
                    if flags_contain(self.stems.get(&stem).map(String::as_str).unwrap_or(""), &rule.flag, self.affix.kind)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn unknown_in(&self, text: &str) -> Vec<String> {
        self.misspelled_tokens(text)
            .into_iter()
            .map(|token| token.word)
            .collect()
    }

    /// Java spell-marker token shape with browser-compatible UTF-16 offsets.
    pub fn misspelled_tokens(&self, text: &str) -> Vec<SpellTokenDto> {
        let mut tokens = Vec::new();
        let mut start_byte = None;
        let mut start_utf16 = 0usize;
        let mut utf16_offset = 0usize;

        for (byte_offset, ch) in text.char_indices() {
            if ch.is_alphabetic() {
                if start_byte.is_none() {
                    start_byte = Some(byte_offset);
                    start_utf16 = utf16_offset;
                }
            } else if let Some(start) = start_byte.take() {
                let word = &text[start..byte_offset];
                if word.chars().count() > 2 && !self.is_correct(word) {
                    tokens.push(SpellTokenDto {
                        word: word.to_string(),
                        offset: start_utf16,
                        length: utf16_offset - start_utf16,
                    });
                }
            }
            utf16_offset += ch.len_utf16();
        }
        if let Some(start) = start_byte {
            let word = &text[start..];
            if word.chars().count() > 2 && !self.is_correct(word) {
                tokens.push(SpellTokenDto {
                    word: word.to_string(),
                    offset: start_utf16,
                    length: utf16_offset - start_utf16,
                });
            }
        }
        tokens
    }

    pub fn learn(&mut self, word: &str, project_root: &Path) -> std::io::Result<()> {
        append_word(&project_root.join("omegat").join("learned_words.txt"), word)?;
        self.learned.insert(word.to_lowercase());
        Ok(())
    }

    pub fn ignore(&mut self, word: &str, project_root: &Path) -> std::io::Result<()> {
        append_word(&project_root.join("omegat").join("ignored_words.txt"), word)?;
        self.ignored.insert(word.to_lowercase());
        Ok(())
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

fn load_hunspell_dir(
    dir: &Path,
    dict: &mut HashSet<String>,
    stems: &mut HashMap<String, String>,
    affix: &mut AffixTable,
) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut affixes: HashMap<String, AffixTable> = HashMap::new();
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) == Some("aff") {
            if let Some(table) = parse_aff(&p) {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                merge_affix(affix, &table);
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
            let expand = p.metadata().map(|m| m.len() < 200_000).unwrap_or(true);
            load_dic_file_affixed(&p, dict, stems, table, expand);
        } else if ext == "txt" {
            load_dic_file(&p, dict);
        }
    }
}

fn merge_affix(dest: &mut AffixTable, src: &AffixTable) {
    dest.kind = src.kind;
    for (k, v) in &src.suffixes {
        dest.suffixes.entry(k.clone()).or_default().extend(v.clone());
    }
    for (k, v) in &src.prefixes {
        dest.prefixes.entry(k.clone()).or_default().extend(v.clone());
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
        if parts.len() >= 2 && parts[0] == "FLAG" {
            table.kind = match parts[1] {
                "long" => FlagKind::Long,
                "num" | "numeric" => FlagKind::Num,
                _ => FlagKind::Char,
            };
            continue;
        }
        if parts.len() < 4 {
            continue;
        }
        let kind = parts[0];
        if kind != "SFX" && kind != "PFX" {
            continue;
        }
        let flag = parts[1].to_string();
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
                    flag: flag.clone(),
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
    let mut stems = HashMap::new();
    load_dic_file_affixed(path, dict, &mut stems, None, true);
}

pub fn load_dic_file_affixed(
    path: &Path,
    dict: &mut HashSet<String>,
    stems: &mut HashMap<String, String>,
    aff: Option<&AffixTable>,
    expand: bool,
) {
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
            Some((w, f)) => (w, f.split(['\t', ' ']).next().unwrap_or(f)),
            None => (line.split(['\t', ' ']).next().unwrap_or(line), ""),
        };
        if word.is_empty() {
            continue;
        }
        stems.insert(word.to_lowercase(), flags.to_string());
        if expand {
            expand_word(word, flags, aff, dict);
        } else {
            dict.insert(word.to_lowercase());
        }
    }
}

fn expand_word(word: &str, flags: &str, aff: Option<&AffixTable>, dict: &mut HashSet<String>) {
    dict.insert(word.to_lowercase());
    let Some(aff) = aff else { return };
    for flag in split_flags(flags, aff.kind) {
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

fn split_flags(flags: &str, kind: FlagKind) -> Vec<String> {
    match kind {
        FlagKind::Long => flags
            .chars()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|c| c.iter().collect())
            .collect(),
        FlagKind::Num => flags.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        FlagKind::Char => flags.chars().map(|c| c.to_string()).collect(),
    }
}

fn flags_contain(flags: &str, flag: &str, kind: FlagKind) -> bool {
    split_flags(flags, kind).iter().any(|f| f == flag)
}

fn unapply_suffix(word: &str, rule: &AffRule) -> Option<String> {
    let stem = if rule.append.is_empty() {
        word.to_string()
    } else if word.ends_with(&rule.append) {
        word[..word.len() - rule.append.len()].to_string()
    } else {
        return None;
    };
    let restored = format!("{}{}", stem, rule.strip);
    if condition_matches(&restored, &rule.condition, true) {
        Some(restored.to_lowercase())
    } else {
        None
    }
}

fn unapply_prefix(word: &str, rule: &AffRule) -> Option<String> {
    let stem = if rule.append.is_empty() {
        word.to_string()
    } else if word.starts_with(&rule.append) {
        word[rule.append.len()..].to_string()
    } else {
        return None;
    };
    let restored = format!("{}{}", rule.strip, stem);
    if condition_matches(&restored, &rule.condition, false) {
        Some(restored.to_lowercase())
    } else {
        None
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
    install_lang(lang, dest).unwrap_or(false)
}

/// Fallible config-scoped variant used by the RPC boundary.
///
/// A missing bundled language is a normal `false`; a destination write failure
/// is an error and must not be reported as a successful install.
pub fn install_lang(lang: &str, dest: &Path) -> std::io::Result<bool> {
    let tag = lang.replace('_', "-").to_lowercase();
    let stem = tag.split('-').next().unwrap_or(&tag);
    std::fs::create_dir_all(dest)?;
    cleanup_install_staging(stem, dest)?;
    if dest.join(format!("{stem}.aff")).exists() && dest.join(format!("{stem}.dic")).exists() {
        return Ok(true);
    }
    let Some((aff, dic)) = reference_dict_paths(stem).or_else(|| resources_dict_paths(stem)) else {
        return Ok(false);
    };
    if !aff.exists() || !dic.exists() {
        return Ok(false);
    }
    let sequence = INSTALL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging = dest.join(format!(
        ".{stem}.{}.{}.staging",
        std::process::id(),
        sequence
    ));
    std::fs::create_dir(&staging)?;
    File::open(dest)?.sync_all()?;
    let staged_aff = staging.join(format!("{stem}.aff"));
    let staged_dic = staging.join(format!("{stem}.dic"));
    let publish = (|| {
        std::fs::copy(&aff, &staged_aff)?;
        std::fs::copy(&dic, &staged_dic)?;
        File::open(&staged_aff)?.sync_all()?;
        File::open(&staged_dic)?.sync_all()?;
        File::open(&staging)?.sync_all()?;
        spell_install_checkpoint(stem, "after_staging_fsync")?;

        let installed_aff = dest.join(format!("{stem}.aff"));
        if installed_aff.exists() {
            std::fs::remove_file(&staged_aff)?;
        } else {
            std::fs::rename(&staged_aff, &installed_aff)?;
        }
        spell_install_checkpoint(stem, "after_aff_rename")?;

        let installed_dic = dest.join(format!("{stem}.dic"));
        if installed_dic.exists() {
            std::fs::remove_file(&staged_dic)?;
        } else {
            std::fs::rename(&staged_dic, &installed_dic)?;
        }
        File::open(dest)?.sync_all()?;
        spell_install_checkpoint(stem, "after_parent_fsync")
    })();
    if let Err(error) = publish {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    std::fs::remove_dir(&staging)?;
    File::open(dest)?.sync_all()?;
    Ok(dest.join(format!("{stem}.aff")).exists() && dest.join(format!("{stem}.dic")).exists())
}

fn cleanup_install_staging(stem: &str, dest: &Path) -> std::io::Result<()> {
    let prefix = format!(".{stem}.");
    let mut removed = false;
    for entry in std::fs::read_dir(dest)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_dir() && name.starts_with(&prefix) && name.ends_with(".staging") {
            std::fs::remove_dir_all(entry.path())?;
            removed = true;
        }
    }
    if removed {
        File::open(dest)?.sync_all()?;
    }
    Ok(())
}

fn spell_install_checkpoint(stem: &str, point: &str) -> std::io::Result<()> {
    if std::env::var("OMEGAT_TEST_SPELL_INSTALL_LANG").as_deref() != Ok(stem)
        || std::env::var("OMEGAT_TEST_SPELL_INSTALL_POINT").as_deref() != Ok(point)
    {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_SPELL_INSTALL_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&marker)?;
    writeln!(
        file,
        "{{\"lang\":{},\"point\":{},\"process_id\":{}}}",
        serde_json::to_string(stem).unwrap(),
        serde_json::to_string(point).unwrap(),
        std::process::id()
    )?;
    file.sync_all()?;
    if let Some(parent) = marker.parent() {
        File::open(parent)?.sync_all()?;
    }
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Java `SpellCheckerManager.getCurrentSpellChecker`: empty plugin list → dummy.
pub fn current_spell_checker(plugin_classes: &[&str]) -> &'static str {
    if plugin_classes
        .iter()
        .any(|c| c.contains("CustomSpellChecker"))
    {
        "custom"
    } else {
        "dummy"
    }
}

/// Java `SpellCheckerManager.getDefaultDictionaryDir` folder name (`OConsts.SPELLING_DICT_DIR`).
pub fn default_dictionary_dir() -> &'static str {
    "dictionary"
}

/// Registered Hunspell dictionary languages (test registers `"dummy"`).
pub fn hunspell_dictionary_languages(registered: &[&str]) -> Vec<String> {
    registered.iter().map(|s| (*s).to_string()).collect()
}

/// Registered Morfologik dictionary languages (test registers `"dummy"`).
pub fn morfologik_dictionary_languages(registered: &[&str]) -> Vec<String> {
    registered.iter().map(|s| (*s).to_string()).collect()
}

/// Language-module stems that must have an `.aff`/`.dic` pair after `ensure_lang`.
pub const LANGUAGE_MODULE_STEMS: &[&str] = &[
    "ar", "ast", "be", "br", "ca", "da", "de", "el", "en", "eo", "es", "fa", "fr", "ga", "gl", "it",
    "ja", "km", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "ta", "tl", "uk", "zh",
];

fn resources_dict_paths(stem: &str) -> Option<(PathBuf, PathBuf)> {
    let roots = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/languages/hunspell"),
        PathBuf::from("resources/languages/hunspell"),
    ];
    for root in roots {
        let aff = root.join(format!("{stem}.aff"));
        let dic = root.join(format!("{stem}.dic"));
        if aff.exists() && dic.exists() {
            return Some((aff, dic));
        }
    }
    None
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
    let mut contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    contents.extend_from_slice(word.as_bytes());
    contents.push(b'\n');
    crate::durable_file::replace(path, &contents)
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
        let mut stems = HashMap::new();
        let mut aff = AffixTable::default();
        load_hunspell_dir(&root.join("hunspell"), &mut hun_set, &mut stems, &mut aff);
        let mut luc_set = HashSet::new();
        load_hunspell_dir(&root.join("lucene"), &mut luc_set, &mut HashMap::new(), &mut AffixTable::default());
        let mut mor_set = HashSet::new();
        load_hunspell_dir(&root.join("morfologik"), &mut mor_set, &mut HashMap::new(), &mut AffixTable::default());
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
        s.learn("OmegaT", dir.path()).unwrap();
        s.ignore("Ctrl", dir.path()).unwrap();
        assert!(s.is_correct("OmegaT"));
        assert!(s.is_correct("Ctrl"));
    }

    #[test]
    fn word_lists_are_atomically_replaced_without_changing_append_order() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("omegat/learned_words.txt");
        append_word(&path, "first").unwrap();
        append_word(&path, "second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first\nsecond\n");
        assert!(std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn language_module_fr_detects_real_misspelling() {
        let dest = tempdir().unwrap();
        assert!(
            ensure_lang("fr", dest.path()),
            "reference/java/language-modules fr aff/dic must be copied"
        );
        let mut dict = HashSet::new();
        let mut stems = HashMap::new();
        let mut aff = AffixTable::default();
        load_hunspell_dir(dest.path(), &mut dict, &mut stems, &mut aff);
        let s = SpellChecker {
            backend: SpellBackend::Hunspell,
            dictionary: dict,
            stems,
            affix: aff,
            ..SpellChecker::default()
        };
        assert!(s.is_correct("maison") || s.is_correct("bonjour"), "fr stems loaded");
        assert!(!s.is_correct("xyzzyqqfr"), "real misspelling must be flagged");
    }

    #[test]
    fn all_language_module_stems_have_aff_dic() {
        for stem in LANGUAGE_MODULE_STEMS {
            let dest = tempdir().unwrap();
            assert!(
                ensure_lang(stem, dest.path()),
                "{stem}: need reference/java or resources/languages/hunspell aff/dic"
            );
            assert!(dest.path().join(format!("{stem}.aff")).exists());
            assert!(dest.path().join(format!("{stem}.dic")).exists());
        }
    }
}
