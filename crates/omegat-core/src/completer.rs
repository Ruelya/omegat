//! Auto-completer views aligned with Java Glossary / Autotext / CharTable /
//! HistoryCompleter / HistoryPredictor / Tag.

use omegat_ipc::CompleterItemDto;
use std::collections::HashMap;

pub fn history_complete(translations: &[&str], prefix: &str) -> Vec<CompleterItemDto> {
    if prefix.is_empty() {
        return vec![];
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let p = prefix.to_lowercase();
    for text in translations {
        for w in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
            if w.len() > 1 && w.to_lowercase().starts_with(&p) && w.to_lowercase() != p && seen.insert(w.to_string())
            {
                out.push(CompleterItemDto {
                    kind: "history".into(),
                    text: w.to_string(),
                    detail: "history-completer".into(),
                });
            }
        }
    }
    out
}

/// Train next-word frequencies from completed translations (Java `WordPredictor`).
pub fn train_predictor(translations: &[&str]) -> HashMap<String, HashMap<String, u32>> {
    let mut model: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for text in translations {
        let words: Vec<&str> = text
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .collect();
        for pair in words.windows(2) {
            *model
                .entry(pair[0].to_lowercase())
                .or_default()
                .entry(pair[1].to_string())
                .or_default() += 1;
        }
    }
    model
}

/// Predict the next word after the last *completed* token. Not a prefix search
/// over the translation vocabulary.
pub fn history_predict(model: &HashMap<String, HashMap<String, u32>>, prev_text: &str) -> Vec<CompleterItemDto> {
    let (seed, context) = last_full_word(prev_text);
    if seed.is_empty() {
        return vec![];
    }
    let Some(nexts) = model.get(&seed.to_lowercase()) else {
        return vec![];
    };
    let total: u32 = nexts.values().sum();
    let mut pairs: Vec<_> = nexts.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    pairs
        .into_iter()
        .filter(|(w, _)| context.is_empty() || (w.starts_with(&context) && w.as_str() != context))
        .map(|(w, n)| CompleterItemDto {
            kind: "history-predict".into(),
            text: w.clone(),
            detail: format!("{}%", ((*n as f64 / total as f64) * 100.0).round()),
        })
        .collect()
}

fn last_full_word(prev: &str) -> (String, String) {
    let trailing_space = prev.ends_with(char::is_whitespace);
    let tokens: Vec<&str> = prev
        .split(|c: char| c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    if trailing_space {
        (tokens.last().unwrap_or(&"").to_string(), String::new())
    } else if tokens.len() >= 2 {
        (tokens[tokens.len() - 2].to_string(), tokens[tokens.len() - 1].to_string())
    } else {
        (String::new(), tokens.last().unwrap_or(&"").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictor_is_not_prefix_of_translations() {
        let model = train_predictor(&["Hello world today", "Hello there"]);
        // After "Hello " we predict world/there — not every word starting with H.
        let hits = history_predict(&model, "Hello ");
        let words: Vec<_> = hits.iter().map(|h| h.text.as_str()).collect();
        assert!(words.contains(&"world") || words.contains(&"there"), "{words:?}");
        assert!(!hits.iter().any(|h| h.text == "Hello"));
        let filtered = history_predict(&model, "Hello w");
        assert!(filtered.iter().all(|h| h.text.starts_with('w')));
        assert!(!filtered.iter().any(|h| h.text == "there"));
    }

    #[test]
    fn completer_uses_prefix_of_seen_words() {
        let hits = history_complete(&["Bonjour le monde"], "mon");
        assert_eq!(hits[0].text, "monde");
        assert_eq!(hits[0].kind, "history");
    }
}
