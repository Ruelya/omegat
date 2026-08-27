//! Per-analyzer stem/normalize pipelines.
//!
//! Snowball languages use `rust-stemmers` (same Tartarus algorithms Lucene
//! wraps). Lucene-specific Light / 3.0 / Arabic / Hindi / … stemmers are
//! ports of the Java classes in `org.apache.lucene.analysis.*`.

use once_cell::sync::Lazy;
use rust_stemmers::{Algorithm, Stemmer};

static SNOW_DA: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Danish));
static SNOW_NL: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Dutch));
static SNOW_EN: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::English));
static SNOW_FI: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Finnish));
static SNOW_FR: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::French));
static SNOW_HU: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Hungarian));
static SNOW_IT: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Italian));
static SNOW_NO: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Norwegian));
static SNOW_RO: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Romanian));
static SNOW_RU: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Russian));
static SNOW_SV: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Swedish));
static SNOW_TR: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Turkish));
static SNOW_EL: Lazy<Stemmer> = Lazy::new(|| Stemmer::create(Algorithm::Greek));

fn snow(stemmer: &Stemmer, word: &str) -> String {
    stemmer.stem(&fold_ascii_lang(word)).into_owned()
}

fn fold_ascii_lang(word: &str) -> String {
    word.replace('İ', "i")
        .replace('I', "ı")
        .to_lowercase()
        .replace("i\u{307}", "i")
}

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
    snow(&SNOW_EN, word)
}

pub fn turkish(word: &str) -> String {
    snow(&SNOW_TR, &turkish_lower(word))
}

pub fn danish(word: &str) -> String {
    snow(&SNOW_DA, word)
}

pub fn dutch(word: &str) -> String {
    snow(&SNOW_NL, word)
}

pub fn finnish(word: &str) -> String {
    snow(&SNOW_FI, word)
}

pub fn hungarian(word: &str) -> String {
    snow(&SNOW_HU, word)
}

pub fn norwegian(word: &str) -> String {
    snow(&SNOW_NO, word)
}

pub fn romanian(word: &str) -> String {
    snow(&SNOW_RO, word)
}

pub fn russian(word: &str) -> String {
    snow(&SNOW_RU, word)
}

pub fn swedish(word: &str) -> String {
    snow(&SNOW_SV, word)
}

pub fn french_light(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    if len > 5 && s[len - 1] == 'x' {
        if len >= 4 && s[len - 3] == 'a' && s[len - 2] == 'u' && s[len - 4] != 'e' {
            s[len - 2] = 'l';
        }
        len -= 1;
    }
    if len > 3 && s[len - 1] == 'x' {
        len -= 1;
    }
    if len > 3 && s[len - 1] == 's' {
        len -= 1;
    }
    if len > 9 && ends_with(&s, len, "issement") {
        len -= 6;
        s[len - 1] = 'r';
        return norm_fr(&mut s, len);
    }
    if len > 8 && ends_with(&s, len, "issant") {
        len -= 4;
        s[len - 1] = 'r';
        return norm_fr(&mut s, len);
    }
    if len > 6 && ends_with(&s, len, "ement") {
        len -= 4;
        if len > 3 && ends_with(&s, len, "ive") {
            len -= 1;
            s[len - 1] = 'f';
        }
        return norm_fr(&mut s, len);
    }
    if len > 8 && ends_with(&s, len, "ation") {
        return norm_fr(&mut s, len - 5);
    }
    if len > 8 && ends_with(&s, len, "ique") {
        len -= 4;
    }
    if len > 5 && ends_with(&s, len, "euse") {
        return norm_fr(&mut s, len - 2);
    }
    if len > 6 && ends_with(&s, len, "teur") {
        len -= 1;
        s[len - 1] = 'r';
        return norm_fr(&mut s, len);
    }
    if len > 7 && ends_with(&s, len, "ive") {
        len -= 1;
        s[len - 1] = 'f';
        return norm_fr(&mut s, len);
    }
    norm_fr(&mut s, len)
}

fn norm_fr(s: &mut [char], mut len: usize) -> String {
    if len > 4 {
        for item in s.iter_mut().take(len) {
            *item = match *item {
                'à' | 'á' | 'â' => 'a',
                'ô' => 'o',
                'è' | 'é' | 'ê' => 'e',
                'ù' | 'û' => 'u',
                'î' => 'i',
                'ç' => 'c',
                c => c,
            };
        }
        let mut i = 1;
        while i < len {
            if s[i] == s[i - 1] && s[i].is_alphabetic() {
                s.copy_within(i + 1..len, i);
                len -= 1;
            } else {
                i += 1;
            }
        }
    }
    if len > 4 && ends_with(s, len, "ie") {
        len -= 2;
    }
    if len > 4 {
        if s[len - 1] == 'r' {
            len -= 1;
        }
        if len > 0 && s[len - 1] == 'e' {
            len -= 1;
        }
        if len > 0 && s[len - 1] == 'e' {
            len -= 1;
        }
        if len > 1 && s[len - 1] == s[len - 2] && s[len - 1].is_alphabetic() {
            len -= 1;
        }
    }
    s[..len].iter().collect()
}

pub fn french(word: &str, full: bool) -> String {
    let light = french_light(word);
    if full {
        snow(&SNOW_FR, &light)
    } else {
        light
    }
}

pub fn italian_light(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let len = s.len();
    if len < 6 {
        return s.into_iter().collect();
    }
    for c in &mut s {
        *c = fold_romance(*c);
    }
    let cut = match s[len - 1] {
        'e' if s[len - 2] == 'i' || s[len - 2] == 'h' => len - 2,
        'e' => len - 1,
        'i' if s[len - 2] == 'h' || s[len - 2] == 'i' => len - 2,
        'i' => len - 1,
        'a' | 'o' if s[len - 2] == 'i' => len - 2,
        'a' | 'o' => len - 1,
        _ => len,
    };
    s[..cut].iter().collect()
}

pub fn italian_snowball(word: &str) -> String {
    snow(&SNOW_IT, word)
}

#[allow(dead_code)]
pub fn italian(word: &str, full: bool) -> String {
    if full {
        italian_snowball(word)
    } else {
        italian_light(word)
    }
}

#[allow(dead_code)]
pub fn spanish(word: &str) -> String {
    spanish_light(word)
}

#[allow(dead_code)]
pub fn portuguese(word: &str) -> String {
    portuguese_light(word)
}

pub fn spanish_light(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    if len < 5 {
        return s.into_iter().collect();
    }
    for c in &mut s {
        *c = fold_romance(*c);
    }
    match s[len - 1] {
        'o' | 'a' | 'e' => len -= 1,
        's' => {
            if len >= 4 && s[len - 2] == 'e' && s[len - 3] == 's' && s[len - 4] == 'e' {
                len -= 2;
            } else if len >= 3 && s[len - 2] == 'e' && s[len - 3] == 'c' {
                s[len - 3] = 'z';
                len -= 2;
            } else if matches!(s[len - 2], 'o' | 'a' | 'e') {
                len -= 2;
            }
        }
        _ => {}
    }
    s[..len].iter().collect()
}

pub fn portuguese_light(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    if len < 4 {
        return s.into_iter().collect();
    }
    len = pt_remove_suffix(&mut s, len);
    if len > 3 && s[len - 1] == 'a' {
        len = pt_norm_feminine(&mut s, len);
    }
    if len > 4 && matches!(s[len - 1], 'e' | 'a' | 'o') {
        len -= 1;
    }
    for c in s.iter_mut().take(len) {
        *c = match *c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ç' => 'c',
            c => c,
        };
    }
    s[..len].iter().collect()
}

fn pt_remove_suffix(s: &mut [char], len: usize) -> usize {
    if len > 4 && ends_with(s, len, "es") && matches!(s[len - 3], 'r' | 's' | 'l' | 'z') {
        return len - 2;
    }
    if len > 3 && ends_with(s, len, "ns") {
        s[len - 2] = 'm';
        return len - 1;
    }
    if len > 4 && (ends_with(s, len, "eis") || ends_with(s, len, "éis")) {
        s[len - 3] = 'e';
        s[len - 2] = 'l';
        return len - 1;
    }
    if len > 4 && ends_with(s, len, "ais") {
        s[len - 2] = 'l';
        return len - 1;
    }
    if len > 4 && ends_with(s, len, "óis") {
        s[len - 3] = 'o';
        s[len - 2] = 'l';
        return len - 1;
    }
    if len > 4 && ends_with(s, len, "is") {
        s[len - 1] = 'l';
        return len;
    }
    if len > 3 && (ends_with(s, len, "ões") || ends_with(s, len, "ães")) {
        s[len - 3] = 'ã';
        s[len - 2] = 'o';
        return len - 1;
    }
    if len > 6 && ends_with(s, len, "mente") {
        return len - 5;
    }
    if len > 3 && s[len - 1] == 's' {
        return len - 1;
    }
    len
}

fn pt_norm_feminine(s: &mut [char], len: usize) -> usize {
    if len > 6
        && (ends_with(s, len, "osa")
            || ends_with(s, len, "ica")
            || ends_with(s, len, "ida")
            || ends_with(s, len, "ada")
            || ends_with(s, len, "iva")
            || ends_with(s, len, "ama"))
    {
        s[len - 1] = 'o';
        return len;
    }
    if len > 6 && ends_with(s, len, "ora") {
        return len - 1;
    }
    if len > 6 && ends_with(s, len, "na") {
        s[len - 1] = 'o';
        return len;
    }
    len
}

/// Lucene `BrazilianStemmer` (RSLP-style), not Portuguese Light.
pub fn brazilian(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    if len > 4 && (ends_with(&s, len, "ões") || ends_with(&s, len, "ães")) {
        s[len - 3] = 'o';
        len -= 2;
    } else if len > 4 && (ends_with(&s, len, "ês") || ends_with(&s, len, "és")) {
        len -= 2;
    } else if len > 4 && (ends_with(&s, len, "ado") || ends_with(&s, len, "ido")) {
        len -= 3;
    } else if len > 4 && (ends_with(&s, len, "adas") || ends_with(&s, len, "idos") || ends_with(&s, len, "idas"))
    {
        len -= 4;
    } else if len > 3 && ends_with(&s, len, "as") {
        len -= 2;
    } else if len > 3 && ends_with(&s, len, "os") {
        len -= 2;
    } else if len > 3 && matches!(s[len - 1], 'a' | 'o' | 'e' | 'á' | 'é' | 'í' | 'ó' | 'ú') {
        len -= 1;
    }
    if len > 2 {
        for c in s.iter_mut().take(len) {
            *c = match *c {
                'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
                'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
                'è' | 'é' | 'ê' | 'ë' => 'e',
                'ù' | 'ú' | 'û' | 'ü' => 'u',
                'ì' | 'í' | 'î' | 'ï' => 'i',
                'ç' => 'c',
                c => c,
            };
        }
    }
    s[..len].iter().collect()
}

/// Lucene 3.0 `GermanStemmer` (Caumanns), used by OmegaT `Lucene30GermanAnalyzer`.
pub fn german_lucene30(word: &str) -> String {
    let term = word.to_lowercase();
    if !term.chars().all(|c| c.is_alphabetic()) {
        return term;
    }
    let mut sb: Vec<char> = term.chars().collect();
    let mut subst = 0usize;
    substitute_de(&mut sb, &mut subst);
    strip_de(&mut sb, subst);
    if !sb.is_empty() && *sb.last().unwrap() == 'z' {
        let last = sb.len() - 1;
        sb[last] = 'x';
    }
    resubstitute_de(&mut sb);
    if sb.len() > 4 {
        if let Some(i) = sb.windows(4).position(|w| w == ['g', 'e', 'g', 'e']) {
            sb.drain(i..i + 2);
        }
    }
    sb.into_iter().collect()
}

fn substitute_de(buf: &mut Vec<char>, subst: &mut usize) {
    let mut c = 0;
    while c < buf.len() {
        if c > 0 && buf[c] == buf[c - 1] {
            buf[c] = '*';
        } else if buf[c] == 'ä' {
            buf[c] = 'a';
        } else if buf[c] == 'ö' {
            buf[c] = 'o';
        } else if buf[c] == 'ü' {
            buf[c] = 'u';
        } else if buf[c] == 'ß' {
            buf[c] = 's';
            buf.insert(c + 1, 's');
            *subst += 1;
        }
        if c < buf.len() - 1 {
            if c + 2 < buf.len() && buf[c] == 's' && buf[c + 1] == 'c' && buf[c + 2] == 'h' {
                buf[c] = '$';
                buf.drain(c + 1..c + 3);
                *subst += 2;
            } else if buf[c] == 'c' && buf[c + 1] == 'h' {
                buf[c] = '§';
                buf.remove(c + 1);
                *subst += 1;
            } else if buf[c] == 'e' && buf[c + 1] == 'i' {
                buf[c] = '%';
                buf.remove(c + 1);
                *subst += 1;
            } else if buf[c] == 'i' && buf[c + 1] == 'e' {
                buf[c] = '&';
                buf.remove(c + 1);
                *subst += 1;
            } else if buf[c] == 'i' && buf[c + 1] == 'g' {
                buf[c] = '#';
                buf.remove(c + 1);
                *subst += 1;
            } else if buf[c] == 's' && buf[c + 1] == 't' {
                buf[c] = '!';
                buf.remove(c + 1);
                *subst += 1;
            }
        }
        c += 1;
    }
}

fn strip_de(buf: &mut Vec<char>, subst: usize) {
    let mut more = true;
    while more && buf.len() > 3 {
        let n = buf.len();
        if n + subst > 5 && buf[n - 2] == 'n' && buf[n - 1] == 'd' {
            buf.truncate(n - 2);
        } else if n + subst > 4 && buf[n - 2] == 'e' && buf[n - 1] == 'm' {
            buf.truncate(n - 2);
        } else if n + subst > 4 && buf[n - 2] == 'e' && buf[n - 1] == 'r' {
            buf.truncate(n - 2);
        } else if buf[n - 1] == 'e' || buf[n - 1] == 's' || buf[n - 1] == 'n' || buf[n - 1] == 't' {
            buf.pop();
        } else {
            more = false;
        }
    }
}

fn resubstitute_de(buf: &mut Vec<char>) {
    let mut c = 0;
    while c < buf.len() {
        match buf[c] {
            '*' => buf[c] = buf[c - 1],
            '$' => {
                buf[c] = 's';
                buf.insert(c + 1, 'c');
                buf.insert(c + 2, 'h');
            }
            '§' => {
                buf[c] = 'c';
                buf.insert(c + 1, 'h');
            }
            '%' => {
                buf[c] = 'e';
                buf.insert(c + 1, 'i');
            }
            '&' => {
                buf[c] = 'i';
                buf.insert(c + 1, 'e');
            }
            '#' => {
                buf[c] = 'i';
                buf.insert(c + 1, 'g');
            }
            '!' => {
                buf[c] = 's';
                buf.insert(c + 1, 't');
            }
            _ => {}
        }
        c += 1;
    }
}

pub fn arabic(word: &str) -> String {
    let norm = arabic_normalize(word);
    arabic_stem(&norm)
}

pub fn arabic_normalize(word: &str) -> String {
    let mut out = String::new();
    for ch in word.chars() {
        match ch {
            '\u{0622}' | '\u{0623}' | '\u{0625}' => out.push('\u{0627}'),
            '\u{0649}' => out.push('\u{064A}'),
            '\u{0629}' => out.push('\u{0647}'),
            '\u{0640}' | '\u{064B}' | '\u{064C}' | '\u{064D}' | '\u{064E}' | '\u{064F}'
            | '\u{0650}' | '\u{0651}' | '\u{0652}' => {}
            c => out.push(c),
        }
    }
    out
}

pub fn arabic_stem(word: &str) -> String {
    let mut s: Vec<char> = word.chars().collect();
    let mut len = s.len();
    const PREFIXES: &[&str] = &["ال", "وال", "بال", "كال", "فال", "لل", "و"];
    const SUFFIXES: &[&str] = &["ها", "ان", "ات", "ون", "ين", "يه", "ية", "ه", "ة", "ي"];
    for p in PREFIXES {
        let pc: Vec<char> = p.chars().collect();
        let min = if pc.len() == 1 { 4 } else { pc.len() + 2 };
        if len >= min && s.starts_with(&pc) {
            s.drain(0..pc.len());
            len = s.len();
            break;
        }
    }
    for suf in SUFFIXES {
        let sc: Vec<char> = suf.chars().collect();
        if len >= sc.len() + 2 && s[len - sc.len()..] == sc[..] {
            len -= sc.len();
            s.truncate(len);
        }
    }
    s.into_iter().collect()
}

pub fn persian(word: &str) -> String {
    let ar = arabic_normalize(word);
    ar.chars()
        .filter_map(|c| match c {
            '\u{06CC}' | '\u{06D2}' => Some('\u{064A}'),
            '\u{06A9}' => Some('\u{0643}'),
            '\u{06C0}' | '\u{06C1}' => Some('\u{0647}'),
            '\u{0654}' => None,
            c => Some(c),
        })
        .collect()
}

pub fn hindi(word: &str) -> String {
    let norm = hindi_normalize(word);
    hindi_stem(&norm)
}

fn hindi_normalize(word: &str) -> String {
    let mut s: Vec<char> = word.chars().collect();
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            '\u{0928}' if i + 1 < s.len() && s[i + 1] == '\u{094D}' => {
                s[i] = '\u{0902}';
                s.remove(i + 1);
            }
            '\u{0901}' => s[i] = '\u{0902}',
            '\u{093C}' | '\u{200C}' | '\u{200D}' => {
                s.remove(i);
                continue;
            }
            '\u{094D}' => {
                s.remove(i);
                continue;
            }
            '\u{0940}' => s[i] = '\u{093F}',
            '\u{0942}' => s[i] = '\u{0941}',
            '\u{0948}' => s[i] = '\u{0947}',
            '\u{094C}' => s[i] = '\u{094B}',
            _ => {}
        }
        i += 1;
    }
    s.into_iter().collect()
}

fn hindi_stem(word: &str) -> String {
    let s: Vec<char> = word.chars().collect();
    let len = s.len();
    let as_str: String = s.iter().collect();
    let suffixes5 = ["ाएंगी", "ाएंगे", "ाऊंगी", "ाऊंगा", "ाइयाँ", "ाइयों", "ाइयां"];
    let suffixes4 = [
        "ाएगी", "ाएगा", "ाओगी", "ाओगे", "एंगी", "ेंगी", "एंगे", "ेंगे", "ूंगी", "ूंगा", "ातीं",
        "नाओं", "नाएं", "ताओं", "ताएं", "ियाँ", "ियों", "ियां",
    ];
    let suffixes3 = [
        "ाकर", "ाइए", "ाईं", "ाया", "ेगी", "ेगा", "ोगी", "ोगे", "ाने", "ाना", "ाते", "ाती", "ाता",
        "तीं", "ाओं", "ाएं", "ुओं", "ुएं", "ुआं",
    ];
    let suffixes2 = [
        "कर", "ाओ", "िए", "ाई", "ाए", "ने", "नी", "ना", "ते", "ीं", "ती", "ता", "ाँ", "ां", "ों", "ें",
    ];
    let suffixes1 = ["ो", "े", "ू", "ु", "ी", "ि", "ा"];
    if len > 6 {
        for suf in suffixes5 {
            if as_str.ends_with(suf) {
                return as_str[..as_str.len() - suf.len()].to_string();
            }
        }
    }
    if len > 5 {
        for suf in suffixes4 {
            if as_str.ends_with(suf) {
                return as_str[..as_str.len() - suf.len()].to_string();
            }
        }
    }
    if len > 4 {
        for suf in suffixes3 {
            if as_str.ends_with(suf) {
                return as_str[..as_str.len() - suf.len()].to_string();
            }
        }
    }
    if len > 3 {
        for suf in suffixes2 {
            if as_str.ends_with(suf) {
                return as_str[..as_str.len() - suf.len()].to_string();
            }
        }
    }
    if len > 2 {
        for suf in suffixes1 {
            if as_str.ends_with(suf) {
                return as_str[..as_str.len() - suf.len()].to_string();
            }
        }
    }
    as_str
}

pub fn czech(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    len = cz_remove_case(&s, len);
    s.truncate(len);
    if len > 5 && (ends_with(&s, len, "ov") || ends_with(&s, len, "in") || ends_with(&s, len, "ův"))
    {
        len -= 2;
        s.truncate(len);
    }
    if len == 0 {
        return String::new();
    }
    if ends_with(&s, len, "čt") {
        s[len - 2] = 'c';
        s[len - 1] = 'k';
    } else if ends_with(&s, len, "št") {
        s[len - 2] = 's';
        s[len - 1] = 'k';
    } else if matches!(s[len - 1], 'c' | 'č') {
        s[len - 1] = 'k';
    } else if matches!(s[len - 1], 'z' | 'ž') {
        s[len - 1] = 'h';
    } else if len > 1 && s[len - 2] == 'e' {
        s[len - 2] = s[len - 1];
        len -= 1;
        s.truncate(len);
    } else if len > 2 && s[len - 2] == 'ů' {
        s[len - 2] = 'o';
    }
    s.into_iter().collect()
}

fn cz_remove_case(s: &[char], len: usize) -> usize {
    if len > 7 && ends_with(s, len, "atech") {
        return len - 5;
    }
    if len > 6 && (ends_with(s, len, "ětem") || ends_with(s, len, "etem") || ends_with(s, len, "atům"))
    {
        return len - 4;
    }
    if len > 5
        && (ends_with(s, len, "ech")
            || ends_with(s, len, "ich")
            || ends_with(s, len, "ích")
            || ends_with(s, len, "ého")
            || ends_with(s, len, "ěmi")
            || ends_with(s, len, "emi")
            || ends_with(s, len, "ému")
            || ends_with(s, len, "ěte")
            || ends_with(s, len, "ete")
            || ends_with(s, len, "ěti")
            || ends_with(s, len, "eti")
            || ends_with(s, len, "ího")
            || ends_with(s, len, "iho")
            || ends_with(s, len, "ími")
            || ends_with(s, len, "ímu")
            || ends_with(s, len, "imu")
            || ends_with(s, len, "ách")
            || ends_with(s, len, "ata")
            || ends_with(s, len, "aty")
            || ends_with(s, len, "ých")
            || ends_with(s, len, "ama")
            || ends_with(s, len, "ami")
            || ends_with(s, len, "ové")
            || ends_with(s, len, "ovi")
            || ends_with(s, len, "ými"))
    {
        return len - 3;
    }
    if len > 4
        && (ends_with(s, len, "em")
            || ends_with(s, len, "es")
            || ends_with(s, len, "ém")
            || ends_with(s, len, "ím")
            || ends_with(s, len, "ům")
            || ends_with(s, len, "at")
            || ends_with(s, len, "ám")
            || ends_with(s, len, "os")
            || ends_with(s, len, "us")
            || ends_with(s, len, "ým")
            || ends_with(s, len, "mi")
            || ends_with(s, len, "ou"))
    {
        return len - 2;
    }
    if len > 3
        && matches!(
            s[len - 1],
            'a' | 'e' | 'i' | 'o' | 'u' | 'ů' | 'y' | 'á' | 'é' | 'í' | 'ý' | 'ě'
        )
    {
        return len - 1;
    }
    len
}

pub fn bulgarian(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    if len < 4 {
        return s.into_iter().collect();
    }
    if len > 5 && ends_with(&s, len, "ища") {
        len -= 3;
    }
    if len > 6 && ends_with(&s, len, "ият") {
        len -= 3;
    } else if len > 5
        && (ends_with(&s, len, "ът")
            || ends_with(&s, len, "то")
            || ends_with(&s, len, "те")
            || ends_with(&s, len, "та")
            || ends_with(&s, len, "ия"))
    {
        len -= 2;
    } else if len > 4 && ends_with(&s, len, "ят") {
        len -= 2;
    }
    if len > 4 && ends_with(&s, len, "и") {
        len -= 1;
    }
    if len > 3 {
        if ends_with(&s[..len], len, "я") {
            len -= 1;
        }
        if ends_with(&s[..len], len, "а") || ends_with(&s[..len], len, "о") || ends_with(&s[..len], len, "е")
        {
            len -= 1;
        }
    }
    if len > 4 && ends_with(&s[..len], len, "ен") {
        s[len - 2] = 'н';
        len -= 1;
    }
    if len > 5 && s[len - 2] == 'ъ' {
        s[len - 2] = s[len - 1];
        len -= 1;
    }
    s.truncate(len);
    s.into_iter().collect()
}

pub fn latvian(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let vowels = s
        .iter()
        .filter(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'ā' | 'ī' | 'ē' | 'ū'))
        .count();
    let affixes: &[(&str, usize, bool)] = &[
        ("ajiem", 3, false),
        ("ajai", 3, false),
        ("ajam", 2, false),
        ("ajām", 2, false),
        ("ajos", 2, false),
        ("ajās", 2, false),
        ("iem", 2, true),
        ("ajā", 2, false),
        ("ais", 2, false),
        ("ai", 2, false),
        ("ei", 2, false),
        ("ām", 1, false),
        ("am", 1, false),
        ("ēm", 1, false),
        ("īm", 1, false),
        ("im", 1, false),
        ("um", 1, false),
        ("us", 1, true),
        ("as", 1, false),
        ("ās", 1, false),
        ("es", 1, false),
        ("os", 1, true),
        ("ij", 1, false),
        ("īs", 1, false),
        ("ēs", 1, false),
        ("is", 1, false),
        ("ie", 1, false),
        ("u", 1, true),
        ("a", 1, true),
        ("i", 1, true),
        ("e", 1, false),
        ("ā", 1, false),
        ("ē", 1, false),
        ("ī", 1, false),
        ("ū", 1, false),
        ("o", 1, false),
        ("s", 0, false),
        ("š", 0, false),
    ];
    let len = s.len();
    for (aff, vc, pal) in affixes {
        let ac: Vec<char> = aff.chars().collect();
        if vowels > *vc && len >= ac.len() + 3 && s.ends_with(&ac) {
            let mut n = len - ac.len();
            s.truncate(n);
            if *pal {
                n = lv_unpalatalize(&mut s, n);
                s.truncate(n);
            }
            return s.into_iter().collect();
        }
    }
    s.into_iter().collect()
}

fn lv_unpalatalize(s: &mut [char], len: usize) -> usize {
    if len > 0 && matches!(s[len - 1], 'č') {
        s[len - 1] = 'c';
        return len;
    }
    if len > 0 && s[len - 1] == 'ļ' {
        s[len - 1] = 'l';
        return len;
    }
    if len > 0 && s[len - 1] == 'ņ' {
        s[len - 1] = 'n';
        return len;
    }
    len
}

pub fn indonesian(word: &str) -> String {
    let mut s: Vec<char> = word.to_lowercase().chars().collect();
    let mut len = s.len();
    let mut syl = s.iter().filter(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')).count();
    let mut flags = 0u32;
    const REMOVED_KE: u32 = 1;
    const REMOVED_PENG: u32 = 2;
    const REMOVED_DI: u32 = 4;
    const REMOVED_MENG: u32 = 8;
    const REMOVED_TER: u32 = 16;
    const REMOVED_BER: u32 = 32;
    const REMOVED_PE: u32 = 64;
    if syl > 2 && (ends_with(&s, len, "kah") || ends_with(&s, len, "lah") || ends_with(&s, len, "pun"))
    {
        len -= 3;
        syl -= 1;
    }
    if syl > 2 && (ends_with(&s[..len], len, "ku") || ends_with(&s[..len], len, "mu")) {
        len -= 2;
        syl -= 1;
    } else if syl > 2 && ends_with(&s[..len], len, "nya") {
        len -= 3;
        syl -= 1;
    }
    let old = len;
    if syl > 2 {
        if starts_with(&s, len, "meng") {
            flags |= REMOVED_MENG;
            s.drain(0..4);
            len -= 4;
            syl -= 1;
        } else if starts_with(&s, len, "men") {
            flags |= REMOVED_MENG;
            s.drain(0..3);
            len -= 3;
            syl -= 1;
        } else if starts_with(&s, len, "me") {
            flags |= REMOVED_MENG;
            s.drain(0..2);
            len -= 2;
            syl -= 1;
        } else if starts_with(&s, len, "di") {
            flags |= REMOVED_DI;
            s.drain(0..2);
            len -= 2;
            syl -= 1;
        } else if starts_with(&s, len, "ke") {
            flags |= REMOVED_KE;
            s.drain(0..2);
            len -= 2;
            syl -= 1;
        }
    }
    if old != len {
        if syl > 2 && ends_with(&s[..len], len, "kan") && flags & (REMOVED_KE | REMOVED_PENG | REMOVED_PE) == 0
        {
            len -= 3;
            syl -= 1;
        } else if syl > 2
            && ends_with(&s[..len], len, "an")
            && flags & (REMOVED_DI | REMOVED_MENG | REMOVED_TER) == 0
        {
            len -= 2;
            syl -= 1;
        }
        if old != len && syl > 2 && starts_with(&s[..len], len, "ber") {
            flags |= REMOVED_BER;
            s.drain(0..3);
            len -= 3;
        }
    } else {
        if syl > 2 && starts_with(&s[..len], len, "ber") {
            flags |= REMOVED_BER;
            s.drain(0..3);
            len -= 3;
            syl -= 1;
        }
        if syl > 2 && ends_with(&s[..len], len, "an") && flags & (REMOVED_DI | REMOVED_MENG | REMOVED_TER) == 0
        {
            len -= 2;
        }
    }
    s.truncate(len);
    s.into_iter().collect()
}

/// Snowball Armenian suffix family (`ArmenianAnalyzer`).
pub fn armenian(word: &str) -> String {
    let mut s = word.to_lowercase();
    for suf in ["երենը", "երեն", "երով", "երից", "ները", "ներ", "ում", "ենի", "ով", "ից", "ը", "ի"] {
        if s.ends_with(suf) && s.chars().count() > suf.chars().count() + 2 {
            let n = s.chars().count() - suf.chars().count();
            s = s.chars().take(n).collect();
            break;
        }
    }
    s
}

/// Snowball Basque suffix family (`BasqueAnalyzer`).
pub fn basque(word: &str) -> String {
    let mut s = word.to_lowercase();
    for suf in [
        "kuntza", "dunek", "tzen", "tzea", "tzat", "ko", "ka", "ta", "tu", "te", "ten", "ara", "ari",
        "tik", "zko",
    ] {
        if s.ends_with(suf) && s.chars().count() > suf.chars().count() + 2 {
            let n = s.chars().count() - suf.chars().count();
            s = s.chars().take(n).collect();
            break;
        }
    }
    s
}

/// Snowball Catalan (`CatalanAnalyzer`): accent fold + common suffixes.
pub fn catalan(word: &str) -> String {
    let mut s: String = word
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' => 'a',
            'è' | 'é' => 'e',
            'í' | 'ï' => 'i',
            'ò' | 'ó' => 'o',
            'ú' | 'ü' => 'u',
            c => c,
        })
        .collect();
    for suf in ["mente", "acions", "acio", "anca", "enc", "ista", "able", "ible", "ment", "itat", "atiu", "acio"]
    {
        if s.ends_with(suf) && s.len() > suf.len() + 3 {
            s.truncate(s.len() - suf.len());
            break;
        }
    }
    if s.ends_with("ada") && s.len() > 6 {
        s.truncate(s.len() - 3);
    }
    if s.ends_with('s') && s.len() > 4 {
        s.pop();
    }
    if s.ends_with('a') && s.len() > 2 {
        s.pop();
    }
    s
}

pub fn irish(word: &str) -> String {
    let mut s = irish_lower(word);
    // Undo lenition (Bh/Ch/Dh/Fh/Gh/Mh/Ph/Sh/Th) after IrishLowerCaseFilter.
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 1 && chars[1] == 'h' && "bcdfgmpst".contains(chars[0]) {
        s = chars[0].to_string() + &chars[2..].iter().collect::<String>();
    }
    s
}

fn irish_lower(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() > 1
        && matches!(chars[0], 'n' | 't' | 'N' | 'T')
        && matches!(chars[1], 'A' | 'E' | 'I' | 'O' | 'U' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú')
    {
        let mut out = String::new();
        out.push(chars[0].to_ascii_lowercase());
        out.push('-');
        for c in chars.iter().skip(1) {
            out.extend(c.to_lowercase());
        }
        return out;
    }
    word.to_lowercase()
}

pub fn greek(word: &str) -> String {
    let folded: String = word
        .chars()
        .map(|c| {
            let l = c.to_lowercase().next().unwrap_or(c);
            match l {
                'ά' | 'ὰ' | 'ᾶ' | 'ἀ' | 'ἁ' => 'α',
                'έ' | 'ὲ' | 'ἐ' | 'ἑ' => 'ε',
                'ή' | 'ὴ' | 'ῆ' | 'ἠ' | 'ἡ' => 'η',
                'ί' | 'ὶ' | 'ῖ' | 'ἰ' | 'ἱ' | 'ϊ' | 'ΐ' | 'ΐ' => 'ι',
                'ό' | 'ὸ' | 'ὀ' | 'ὁ' => 'ο',
                'ύ' | 'ὺ' | 'ῦ' | 'ὐ' | 'ὑ' | 'ϋ' => 'υ',
                'ώ' | 'ὼ' | 'ῶ' | 'ὠ' | 'ὡ' => 'ω',
                'ς' => 'σ',
                c => c,
            }
        })
        .collect();
    snow(&SNOW_EL, &folded)
}

pub fn galician(word: &str) -> String {
    let mut s: Vec<char> = word
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            c => c,
        })
        .collect();
    // unha → uña (nh → ñ)
    let mut i = 0;
    while i + 1 < s.len() {
        if s[i] == 'n' && s[i + 1] == 'h' {
            s[i] = 'ñ';
            s.remove(i + 1);
        } else {
            i += 1;
        }
    }
    let mut len = s.len();
    if len > 4 && ends_with(&s, len, "ego") {
        len -= 3;
    } else if len > 5 && ends_with(&s, len, "anica") {
        len -= 5;
    } else if len > 4 && ends_with(&s, len, "ica") {
        len -= 3;
    } else if len > 3 && s[len - 1] == 'a' {
        len -= 1;
    }
    s.truncate(len);
    s.into_iter().collect()
}

/// Light Polish inflection strip (Stempel-compatible for case endings).
pub fn polish(word: &str) -> String {
    let mut s = word.to_lowercase();
    if s.ends_with("iem") && s.chars().count() > 5 {
        let n = s.chars().count() - 3;
        return s.chars().take(n).collect();
    }
    if s.ends_with("im") && s.chars().count() > 6 {
        s.pop();
        return s;
    }
    if s.ends_with("ami") && s.chars().count() > 5 {
        let n = s.chars().count() - 3;
        return s.chars().take(n).collect();
    }
    if s.ends_with("ach") && s.chars().count() > 5 {
        let n = s.chars().count() - 3;
        return s.chars().take(n).collect();
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

fn fold_romance(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ä' => 'a',
        'ò' | 'ó' | 'ô' | 'ö' => 'o',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        c => c,
    }
}

fn ends_with(s: &[char], len: usize, suf: &str) -> bool {
    let sc: Vec<char> = suf.chars().collect();
    len >= sc.len() && s[len - sc.len()..len] == sc[..]
}

fn starts_with(s: &[char], len: usize, pre: &str) -> bool {
    let pc: Vec<char> = pre.chars().collect();
    len >= pc.len() && s[..pc.len()] == pc[..]
}

fn measure(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut m = 0;
    let mut prev_v = false;
    for &c in &chars {
        let v = matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if prev_v && !v {
            m += 1;
        }
        prev_v = v;
    }
    m
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
    replace_if_m(
        s,
        &[
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
        ],
        0,
    );
}

fn step_3(s: &mut String) {
    replace_if_m(
        s,
        &[
            ("icate", "ic"),
            ("ative", ""),
            ("alize", "al"),
            ("iciti", "ic"),
            ("ical", "ic"),
            ("ful", ""),
            ("ness", ""),
        ],
        0,
    );
}

fn step_4(s: &mut String) {
    for suf in [
        "al", "ance", "ence", "er", "ic", "able", "ible", "ant", "ement", "ment", "ent", "ion", "ou",
        "ism", "ate", "iti", "ous", "ive", "ize",
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
    !matches!(a, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
        && matches!(b, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
        && !matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'w' | 'x')
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
