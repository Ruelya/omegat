use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellBackend {
    Hunspell,
    Lucene,
    Morfologik,
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
        for dir in language_dirs(project_root, config_dir) {
            load_hunspell_dir(&dir, &mut s.dictionary);
            if backend == SpellBackend::Morfologik {
                load_wordlist_into(&dir.join("pl.dict.txt"), &mut s.dictionary);
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

fn language_dirs(project_root: &Path, config_dir: &Path) -> Vec<PathBuf> {
    [
        project_root.join("omegat").join("spell"),
        config_dir.join("spell"),
        PathBuf::from("resources/languages"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/languages"),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect()
}

fn load_hunspell_dir(dir: &Path, dict: &mut HashSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "dic" || ext == "txt" {
            load_dic_file(&p, dict);
        }
    }
}

/// Hunspell `.dic`: first line is count, then `word/FLAGS`.
pub fn load_dic_file(path: &Path, dict: &mut HashSet<String>) {
    let Ok(raw) = std::fs::read_to_string(path) else { return };
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if i == 0 && line.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let word = line.split(['/', '\t', ' ']).next().unwrap_or(line);
        if !word.is_empty() {
            dict.insert(word.to_lowercase());
        }
    }
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
    fn hunspell_dic_roundtrip() {
        let dir = tempdir().unwrap();
        let dic = dir.path().join("en.dic");
        std::fs::write(&dic, "3\nhello\nworld\nOmegaT/M\n").unwrap();
        let mut set = HashSet::new();
        load_dic_file(&dic, &mut set);
        assert!(set.contains("hello"));
        assert!(set.contains("omegat"));
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
