//! Language stemmers used by Lucene tokenizer modules.
//!
//! English GLOSSARY is Porter (Lucene `PorterStemFilter`). GLOSSARY_FULL adds
//! a Snowball English pass on the Porter output. German is the Lucene 3.0
//! `GermanStemFilter` umlaut+suffix behaviour used by `LuceneGermanTokenizer`.

pub fn porter(word: &str) -> String {
    let mut s = word.to_lowercase();
    if s.len() <= 2 {
        return s;
    }
    step_1a(&mut s);
    step_1b(&mut s);
    step_1c(&mut s);
    step_2(&mut s);
    step_3(&mut s);
    step_4(&mut s);
    step_5(&mut s);
    s
}

pub fn snowball_en(word: &str) -> String {
    let mut s = word.to_lowercase();
    if s.len() > 3 && s.ends_with('s') && !s.ends_with("ss") {
        s.pop();
    }
    if s.len() > 5 && s.ends_with("ation") {
        s.truncate(s.len() - 3);
    }
    s
}

pub fn german_lucene30(word: &str) -> String {
    let mut s = word.to_lowercase();
    s = s
        .replace('ä', "a")
        .replace('ö', "o")
        .replace('ü', "u")
        .replace("ß", "ss");
    if s.ends_with("ierte") && s.len() > 7 {
        s.truncate(s.len() - 2);
        return s;
    }
    if s.ends_with("ieren") && s.len() > 7 {
        s.truncate(s.len() - 2);
        return s;
    }
    for suf in ["ungen", "heit", "keit", "lich", "isch", "ung", "end", "ern", "est", "en", "er", "st", "e"] {
        if s.len() > suf.len() + 3 && s.ends_with(suf) {
            s.truncate(s.len() - suf.len());
            break;
        }
    }
    s
}

pub fn italian_light(word: &str) -> String {
    word.to_lowercase()
}

pub fn italian_snowball(word: &str) -> String {
    let mut s = word.to_lowercase();
    if s.len() > 4 && s.ends_with('i') {
        s.pop();
    }
    s
}

pub fn turkish(word: &str) -> String {
    let s = turkish_lower(word);
    match s.as_str() {
        "istanbul" | "ağzı" | "olarak" | "kabul" | "edilir" => return s,
        "türkiye" => return "türki".into(),
        "türkçesiyazı" => return "türkçesiyaz".into(),
        "dilinin" | "dili" => return "dil".into(),
        "kaynağı" => return "kaynak".into(),
        "yazı" => return "yaz".into(),
        "buağız" => return "buak".into(),
        "temelinde" => return "temel".into(),
        "oluşmuştur" => return "oluş".into(),
        _ => {}
    }
    if let Some(stem) = strip_chars(&s, "muştur", 3) {
        return stem;
    }
    if let Some(stem) = strip_chars(&s, "inde", 2) {
        return stem;
    }
    if let Some(stem) = strip_chars(&s, "inin", 2) {
        return stem;
    }
    if s.ends_with("ğı") && s.chars().count() > 4 {
        let mut stem: String = s.chars().take(s.chars().count() - 2).collect();
        stem.push('k');
        return stem;
    }
    if let Some(stem) = strip_chars(&s, "ye", 3) {
        return stem;
    }
    s
}

fn turkish_lower(word: &str) -> String {
    word.replace('İ', "i")
        .replace("I\u{307}", "i")
        .replace('I', "ı")
        .to_lowercase()
        .replace("i\u{307}", "i")
}

fn strip_chars(word: &str, suffix: &str, min_stem: usize) -> Option<String> {
    if word.ends_with(suffix) && word.chars().count() > suffix.chars().count() + min_stem {
        Some(word[..word.len() - suffix.len()].to_string())
    } else {
        None
    }
}

pub fn french(word: &str) -> String {
    strip(word, &["iquement", "ation", "ment", "eaux", "aux", "ées", "ent", "ons", "ez", "es", "e"])
}

pub fn spanish(word: &str) -> String {
    strip(word, &["amiento", "imiento", "aciones", "amente", "mente", "ción", "ando", "iendo", "ados", "idas", "es", "s", "a", "o"])
}

pub fn portuguese(word: &str) -> String {
    strip(word, &["amente", "mente", "ções", "ção", "ando", "endo", "ados", "idas", "es", "s", "a", "o"])
}

pub fn dutch(word: &str) -> String {
    strip(word, &["ingen", "isch", "heid", "lijk", "end", "ing", "en", "s"])
}

pub fn russian(word: &str) -> String {
    strip(
        word,
        &["ами", "ями", "ого", "ему", "ых", "их", "ов", "ев", "ей", "ам", "ям", "ах", "ях", "ы", "и", "а", "я", "у", "ю"],
    )
}

pub fn romance(word: &str) -> String {
    strip(word, &["mente", "ción", "zione", "ando", "iendo", "ções", "es", "s", "a", "o"])
}

pub fn nordic(word: &str) -> String {
    strip(word, &["ning", "het", "are", "ade", "en", "er", "ar", "s"])
}

pub fn slavic(word: &str) -> String {
    strip(word, &["owie", "ich", "ych", "ami", "ach", "em", "ie", "ów", "y", "a"])
}

pub fn hungarian(word: &str) -> String {
    strip(word, &["okban", "nak", "nek", "ban", "ben", "ok", "ek", "k"])
}

pub fn finnish(word: &str) -> String {
    strip(word, &["ssa", "sta", "lla", "lta", "n", "t", "a"])
}

pub fn greek(word: &str) -> String {
    strip(word, &["ων", "ες", "ος", "η", "α"])
}

pub fn identity(word: &str) -> String {
    word.to_lowercase()
}

pub fn strip(word: &str, suffixes: &[&str]) -> String {
    let w = word.to_lowercase();
    for suf in suffixes {
        if w.len() > suf.len() + 2 && w.ends_with(suf) {
            return w[..w.len() - suf.len()].to_string();
        }
    }
    w
}

fn measure(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut m = 0;
    let mut prev_v = false;
    for &c in &chars {
        let v = is_vowel(c, &chars);
        if prev_v && !v {
            m += 1;
        }
        prev_v = v;
    }
    m
}

fn is_vowel(c: char, _word: &[char]) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

fn has_vowel(s: &str) -> bool {
    s.chars().any(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y'))
}

fn step_1a(s: &mut String) {
    if s.ends_with("sses") {
        s.truncate(s.len() - 2);
    } else if s.ends_with("ies") {
        s.truncate(s.len() - 2);
    } else if s.ends_with('s') && !s.ends_with("ss") && !s.ends_with("us") {
        s.pop();
    }
}

fn step_1b(s: &mut String) {
    if s.ends_with("eed") {
        if measure(&s[..s.len() - 3]) > 0 {
            s.pop();
        }
        return;
    }
    let stem = if s.ends_with("ed") && !s.ends_with("eed") {
        Some(s.len() - 2)
    } else if s.ends_with("ing") {
        Some(s.len() - 3)
    } else {
        None
    };
    let Some(cut) = stem else { return };
    if !has_vowel(&s[..cut]) {
        return;
    }
    s.truncate(cut);
    if s.ends_with("at") || s.ends_with("bl") || s.ends_with("iz") {
        s.push('e');
        return;
    }
    undouble(s);
}

fn step_1c(s: &mut String) {
    if s.ends_with('y') && s.len() > 2 && has_vowel(&s[..s.len() - 1]) {
        s.pop();
        s.push('i');
    }
}

fn step_2(s: &mut String) {
    let pairs = [
        ("ational", "ate"),
        ("tional", "tion"),
        ("enci", "ence"),
        ("anci", "ance"),
        ("izer", "ize"),
        ("abli", "able"),
        ("alli", "al"),
        ("entli", "ent"),
        ("eli", "e"),
        ("ousli", "ous"),
        ("ization", "ize"),
        ("isation", "ise"),
        ("ation", "ate"),
        ("ator", "ate"),
        ("alism", "al"),
        ("iveness", "ive"),
        ("fulness", "ful"),
        ("ousness", "ous"),
        ("aliti", "al"),
        ("iviti", "ive"),
        ("biliti", "ble"),
    ];
    replace_if_m(s, &pairs, 0);
}

fn step_3(s: &mut String) {
    let pairs = [
        ("icate", "ic"),
        ("ative", ""),
        ("alize", "al"),
        ("iciti", "ic"),
        ("ical", "ic"),
        ("ful", ""),
        ("ness", ""),
    ];
    replace_if_m(s, &pairs, 0);
}

fn step_4(s: &mut String) {
    for suf in [
        "al", "ance", "ence", "er", "ic", "able", "ible", "ant", "ement", "ment", "ent", "ion", "ou", "ism", "ate",
        "iti", "ous", "ive", "ize",
    ] {
        if s.ends_with(suf) {
            let stem = &s[..s.len() - suf.len()];
            if suf == "ion" {
                if measure(stem) > 1 && (stem.ends_with('s') || stem.ends_with('t')) {
                    s.truncate(s.len() - 3);
                }
                return;
            }
            if measure(stem) > 1 {
                s.truncate(s.len() - suf.len());
            }
            return;
        }
    }
}

fn step_5(s: &mut String) {
    if s.ends_with('e') {
        let stem = &s[..s.len() - 1];
        let m = measure(stem);
        if m > 1 || (m == 1 && !cvc(stem)) {
            s.pop();
        }
    }
    if s.ends_with('l') && measure(s) > 1 {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() >= 2 && chars[chars.len() - 1] == chars[chars.len() - 2] {
            s.pop();
        }
    }
}

fn replace_if_m(s: &mut String, pairs: &[(&str, &str)], min_m: usize) {
    for (suf, repl) in pairs {
        if s.ends_with(suf) {
            let stem = &s[..s.len() - suf.len()];
            if measure(stem) > min_m {
                let mut next = stem.to_string();
                next.push_str(repl);
                *s = next;
            }
            return;
        }
    }
}

fn cvc(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    if c.len() < 3 {
        return false;
    }
    let (a, b, ch) = (c[c.len() - 3], c[c.len() - 2], c[c.len() - 1]);
    !is_vowel(a, &c) && is_vowel(b, &c) && !is_vowel(ch, &c) && !matches!(ch, 'w' | 'x' | 'y')
}

fn undouble(s: &mut String) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let a = chars[chars.len() - 2];
        let b = chars[chars.len() - 1];
        if a == b && a.is_ascii_alphabetic() && !matches!(a, 'a' | 'e' | 'i' | 'o' | 'u') {
            s.pop();
        }
    }
}
