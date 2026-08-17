use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum SpellBackend {
    Hunspell,
    Lucene,
    Morfologik,
}

#[derive(Debug, Default)]
pub struct SpellChecker {
    pub learned: HashSet<String>,
    pub ignored: HashSet<String>,
    pub dictionary: HashSet<String>,
}

impl SpellChecker {
    pub fn load(project_root: &Path, config_dir: &Path) -> Self {
        let mut s = Self::default();
        s.learned = load_wordlist(&project_root.join("omegat").join("learned_words.txt"));
        s.ignored = load_wordlist(&project_root.join("omegat").join("ignored_words.txt"));
        if s.learned.is_empty() {
            s.learned = load_wordlist(&config_dir.join("learned_words.txt"));
        }
        // Tiny built-in English fallback so the feature is testable without Hunspell dicts.
        for w in ["the", "and", "or", "to", "of", "a", "in", "is", "it", "you", "hello", "world"] {
            s.dictionary.insert(w.into());
        }
        s
    }

    pub fn is_correct(&self, word: &str) -> bool {
        let w = word.to_lowercase();
        if w.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        self.ignored.contains(&w)
            || self.learned.contains(&w)
            || self.dictionary.contains(&w)
            || word.chars().any(|c| !c.is_ascii_alphabetic())
    }

    pub fn unknown_in(&self, text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphabetic())
            .filter(|w| w.len() > 2 && !self.is_correct(w))
            .map(|w| w.to_string())
            .collect()
    }

    pub fn learn(&mut self, word: &str, project_root: &Path) {
        self.learned.insert(word.to_lowercase());
        let path = project_root.join("omegat").join("learned_words.txt");
        let _ = append_word(&path, word);
    }

    pub fn ignore(&mut self, word: &str, project_root: &Path) {
        self.ignored.insert(word.to_lowercase());
        let path = project_root.join("omegat").join("ignored_words.txt");
        let _ = append_word(&path, word);
    }
}

fn load_wordlist(path: &Path) -> HashSet<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

fn append_word(path: &Path, word: &str) -> std::io::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{word}")
}
