use crate::cancellation::CancellationToken;
use flate2::read::GzDecoder;
use omegat_ipc::DictHitDto;
use std::io::Read;
use std::path::Path;

/// StarDict (`.ifo` + `.idx` + `.dict` / `.dict.dz`) and Lingvo DSL (`.dsl` / `.dsl.dz`).
pub fn lookup(dir: &Path, word: &str) -> Vec<DictHitDto> {
    lookup_opts(dir, word, false)
}

pub fn lookup_opts(dir: &Path, word: &str, fuzzy: bool) -> Vec<DictHitDto> {
    lookup_opts_cancellable(dir, word, fuzzy, &CancellationToken::default()).unwrap_or_default()
}

/// Dictionary lookup with request-scoped cooperative cancellation.
///
/// Cancellation is checked around every dictionary file and before a fuzzy
/// retry. A cancelled scan does not publish the partial hit list.
pub fn lookup_opts_cancellable(
    dir: &Path,
    word: &str,
    fuzzy: bool,
    cancellation: &CancellationToken,
) -> Option<Vec<DictHitDto>> {
    if cancellation.is_cancelled() {
        return None;
    }
    if !dir.exists() || word.is_empty() {
        return Some(vec![]);
    }
    let mut hits = Vec::new();
    let needle = word.to_lowercase();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Some(hits);
    };
    for ent in rd.flatten() {
        if cancellation.is_cancelled() {
            return None;
        }
        let p = ent.path();
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if name.ends_with(".dsl") || name.ends_with(".dsl.dz") {
            hits.extend(lookup_dsl(&p, &needle));
        } else if name.ends_with(".ifo") {
            hits.extend(lookup_stardict(&p, &needle));
        }
        if cancellation.is_cancelled() {
            return None;
        }
    }
    if hits.is_empty() && fuzzy && needle.chars().count() >= 3 {
        let prefix: String = needle
            .chars()
            .take(needle.chars().count().saturating_sub(1))
            .collect();
        if !prefix.is_empty() {
            return lookup_opts_cancellable(dir, &prefix, false, cancellation);
        }
    }
    Some(hits)
}

fn lookup_dsl(path: &Path, needle: &str) -> Vec<DictHitDto> {
    let raw = read_dsl_text(path).unwrap_or_default();
    parse_dsl(&raw, needle, &path.display().to_string())
}

/// Lingvo DSL files are often UTF-16 LE (with BOM); gzip `.dsl.dz` may wrap either.
pub fn read_dsl_text(path: &Path) -> Option<String> {
    let bytes = read_maybe_gzip_bytes(path).ok()?;
    Some(decode_dsl_bytes(&bytes))
}

pub fn decode_dsl_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if bytes.len() >= 4 && bytes[0] != 0 && bytes[1] == 0 && bytes[3] == 0 {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn is_dsl_supported(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    (name.ends_with(".dsl") || name.ends_with(".dsl.dz")) && !name.ends_with(".idx")
}

pub fn parse_dsl(raw: &str, needle: &str, source: &str) -> Vec<DictHitDto> {
    read_dsl_articles(raw, source)
        .into_iter()
        .filter(|h| h.word.to_lowercase() == needle || h.word.to_lowercase().contains(needle))
        .collect()
}

pub fn read_dsl_articles(raw: &str, source: &str) -> Vec<DictHitDto> {
    let mut hits = Vec::new();
    let mut heads: Vec<String> = Vec::new();
    let mut def = String::new();
    let flush = |heads: &mut Vec<String>, def: &mut String, hits: &mut Vec<DictHitDto>| {
        if heads.is_empty() {
            return;
        }
        let word = heads.join("\n");
        hits.push(DictHitDto {
            word,
            definition: dsl_markup_to_html(def.trim()),
            source: source.to_string(),
        });
        heads.clear();
        def.clear();
    };
    for line in raw.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            if !def.is_empty() {
                flush(&mut heads, &mut def, &mut hits);
            }
            heads.push(line.trim().to_string());
        } else {
            def.push_str(line.trim());
            def.push('\n');
        }
    }
    flush(&mut heads, &mut def, &mut hits);
    hits
}

pub fn read_dsl_exact(raw: &str, word: &str, source: &str) -> Vec<DictHitDto> {
    let needle = word.to_lowercase();
    read_dsl_articles(raw, source)
        .into_iter()
        .filter(|h| {
            h.word.lines().any(|l| l.to_lowercase() == needle) || h.word.to_lowercase() == needle
        })
        .collect()
}

pub fn read_dsl_predictive(raw: &str, prefix: &str, source: &str) -> Vec<DictHitDto> {
    let needle = prefix.to_lowercase();
    read_dsl_articles(raw, source)
        .into_iter()
        .filter(|h| {
            h.word.to_lowercase().starts_with(&needle)
                || h.word
                    .lines()
                    .any(|l| l.to_lowercase().starts_with(&needle))
        })
        .collect()
}

/// Java dsl4j article HTML used by `LingvoDSLTest`.
pub fn dsl_markup_to_html(raw: &str) -> String {
    let mut s = raw.to_string();
    s = s.replace("\\[", "[").replace("\\]", "]");
    s = replace_paired(
        &s,
        "[m1]",
        "[/m]",
        r#"<div style="text-indent: 30px">"#,
        "</div>",
    );
    s = replace_paired(
        &s,
        "[m2]",
        "[/m]",
        r#"<div style="text-indent: 60px">"#,
        "</div>",
    );
    s = replace_paired(
        &s,
        "[m3]",
        "[/m]",
        r#"<div style="text-indent: 90px">"#,
        "</div>",
    );
    s = replace_paired(&s, "[m]", "[/m]", "<div>", "</div>");
    s = s.replace("[trn]", "").replace("[/trn]", "");
    s = s.replace("[com]", "").replace("[/com]", "");
    s = s.replace("[ref]", "").replace("[/ref]", "");
    s = replace_paired(
        &s,
        "[i]",
        "[/i]",
        "<span style='font-style: italic'>",
        "</span>",
    );
    s = replace_paired(&s, "[b]", "[/b]", "<strong>", "</strong>");
    s = replace_paired(
        &s,
        "[*][ex]",
        "[/ex][/*]",
        r#"<span class="details">"#,
        "</span>",
    );
    s = replace_paired(
        &s,
        r#"[lang name="English"]"#,
        "[/lang]",
        r#"<span class="lang_en">"#,
        "</span>",
    );
    s = s
        .replace("[c][p]", r#"<span style="color: green">"#)
        .replace("[/p][/c]", "</span>");
    s = s
        .replace("[c]", r#"<span style="color: green">"#)
        .replace("[/c]", "</span>");
    s = s.replace("[p]", "").replace("[/p]", "");
    s = replace_t_tags(&s);
    s = s.replace("[/*]", "").replace("[*]", "");
    s.trim().to_string()
}

fn replace_paired(
    input: &str,
    open: &str,
    close: &str,
    html_open: &str,
    html_close: &str,
) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(s) = rest.find(open) {
        out.push_str(&rest[..s]);
        rest = &rest[s + open.len()..];
        if let Some(e) = rest.find(close) {
            out.push_str(html_open);
            out.push_str(&rest[..e]);
            out.push_str(html_close);
            rest = &rest[e + close.len()..];
        } else {
            out.push_str(open);
            break;
        }
    }
    out.push_str(rest);
    out
}

fn replace_t_tags(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(s) = rest.find("[t]") {
        out.push_str(&rest[..s]);
        rest = &rest[s + 3..];
        if let Some(e) = rest.find("[/t]") {
            let mut inner = rest[..e].to_string();
            if inner.starts_with("[[") && inner.ends_with("]]") {
                inner = format!("[{}]", &inner[2..inner.len() - 2]);
            }
            out.push_str(&inner);
            out.push_str("&nbsp;");
            rest = &rest[e + 4..];
        } else {
            out.push_str("[t]");
            break;
        }
    }
    out.push_str(rest);
    out
}

fn lookup_stardict(ifo: &Path, needle: &str) -> Vec<DictHitDto> {
    let stem = ifo.with_extension("");
    let Some(dict) = stardict_dict_path(&stem) else {
        return vec![];
    };
    let Some(idx_bytes) = read_stardict_idx_bytes(ifo) else {
        return vec![];
    };
    let Ok(dict_bytes) = read_maybe_gzip_bytes(&dict) else {
        return vec![];
    };
    parse_stardict_idx(&idx_bytes, &dict_bytes, needle, &ifo.display().to_string())
}

fn read_stardict_idx_bytes(ifo: &Path) -> Option<Vec<u8>> {
    let stem = ifo.with_extension("");
    let idx = stem.with_extension("idx");
    if idx.exists() {
        return std::fs::read(idx).ok();
    }
    let gz = Path::new(&format!("{}.idx.gz", stem.display())).to_path_buf();
    if gz.exists() {
        return read_maybe_gzip_bytes(&gz).ok();
    }
    None
}

pub fn stardict_word_count(ifo: &Path) -> usize {
    let Some(idx_bytes) = read_stardict_idx_bytes(ifo) else {
        return 0;
    };
    parse_stardict_all(&idx_bytes).len()
}

pub fn parse_stardict_all(idx: &[u8]) -> Vec<String> {
    let mut words = Vec::new();
    let mut i = 0;
    while i < idx.len() {
        let Some(z) = idx[i..].iter().position(|&b| b == 0) else {
            break;
        };
        words.push(String::from_utf8_lossy(&idx[i..i + z]).into_owned());
        i += z + 1;
        if i + 8 > idx.len() {
            break;
        }
        i += 8;
    }
    words
}

pub fn read_stardict_articles(ifo: &Path, word: &str, predictive: bool) -> Vec<DictHitDto> {
    let stem = ifo.with_extension("");
    let Some(dict) = stardict_dict_path(&stem) else {
        return vec![];
    };
    let Some(idx_bytes) = read_stardict_idx_bytes(ifo) else {
        return vec![];
    };
    let Ok(dict_bytes) = read_maybe_gzip_bytes(&dict) else {
        return vec![];
    };
    let needle = word.to_lowercase();
    let mut hits = Vec::new();
    let mut i = 0;
    while i < idx_bytes.len() {
        let Some(z) = idx_bytes[i..].iter().position(|&b| b == 0) else {
            break;
        };
        let w = String::from_utf8_lossy(&idx_bytes[i..i + z]).into_owned();
        i += z + 1;
        if i + 8 > idx_bytes.len() {
            break;
        }
        let off = u32::from_be_bytes(idx_bytes[i..i + 4].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(idx_bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        i += 8;
        let wl = w.to_lowercase();
        let ok = if predictive {
            wl.starts_with(&needle)
        } else {
            wl == needle
        };
        if ok && off + size <= dict_bytes.len() {
            let article = String::from_utf8_lossy(&dict_bytes[off..off + size])
                .trim()
                .to_string();
            let wrapped = if article.contains('<') {
                article
            } else {
                format!("<div>{article}</div>")
            };
            hits.push(DictHitDto {
                word: w,
                definition: wrapped,
                source: ifo.display().to_string(),
            });
        }
    }
    hits
}

fn stardict_dict_path(stem: &std::path::Path) -> Option<std::path::PathBuf> {
    let plain = stem.with_extension("dict");
    if plain.exists() {
        return Some(plain);
    }
    let dz = std::path::PathBuf::from(format!("{}.dict.dz", stem.display()));
    if dz.exists() {
        return Some(dz);
    }
    None
}

/// StarDict idx: utf-8 word, NUL, 32-bit offset, 32-bit size (big-endian).
pub fn parse_stardict_idx(idx: &[u8], dict: &[u8], needle: &str, source: &str) -> Vec<DictHitDto> {
    let mut hits = Vec::new();
    let mut i = 0;
    while i < idx.len() {
        let Some(z) = idx[i..].iter().position(|&b| b == 0) else {
            break;
        };
        let word = String::from_utf8_lossy(&idx[i..i + z]).into_owned();
        i += z + 1;
        if i + 8 > idx.len() {
            break;
        }
        let off = u32::from_be_bytes(idx[i..i + 4].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(idx[i + 4..i + 8].try_into().unwrap()) as usize;
        i += 8;
        if word.to_lowercase() == needle && off + size <= dict.len() {
            hits.push(DictHitDto {
                word,
                definition: String::from_utf8_lossy(&dict[off..off + size]).into_owned(),
                source: source.to_string(),
            });
        }
    }
    hits
}

/// Java `org.omegat.core.dictionaries.DictionaryData`.
#[derive(Debug)]
pub struct DictionaryData {
    finalized: bool,
    /// key → values (original + lowercase copies, matching Java `add`).
    store: std::collections::BTreeMap<String, Vec<(String, String)>>,
}

impl DictionaryData {
    pub fn new() -> Self {
        Self {
            finalized: false,
            store: std::collections::BTreeMap::new(),
        }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        let key = crate::string_util::normalize_unicode(key);
        self.do_add(&key, value);
        let lower = key.to_lowercase();
        if lower != key {
            self.do_add(&lower, value);
        }
    }

    fn do_add(&mut self, key: &str, value: &str) {
        self.store
            .entry(key.to_string())
            .or_default()
            .push((key.to_string(), value.to_string()));
    }

    pub fn done(&mut self) {
        self.finalized = true;
    }

    pub fn size(&self) -> i64 {
        if !self.finalized {
            -1
        } else {
            self.store.len() as i64
        }
    }

    pub fn look_up(&self, word: &str) -> Result<Vec<(String, String)>, &'static str> {
        self.do_look_up(word, false)
    }

    pub fn look_up_predictive(&self, word: &str) -> Result<Vec<(String, String)>, &'static str> {
        self.do_look_up(word, true)
    }

    fn do_look_up(
        &self,
        word: &str,
        predictive: bool,
    ) -> Result<Vec<(String, String)>, &'static str> {
        if !self.finalized {
            return Err("not finalized");
        }
        let word = crate::string_util::normalize_unicode(word);
        let mut result = self.collect(&word, predictive);
        if result.is_empty() {
            result = self.collect(&word.to_lowercase(), predictive);
        }
        Ok(result)
    }

    fn collect(&self, word: &str, predictive: bool) -> Vec<(String, String)> {
        if predictive {
            self.store
                .iter()
                .filter(|(k, _)| k.starts_with(word))
                .flat_map(|(_, v)| v.iter().cloned())
                .collect()
        } else {
            self.store.get(word).cloned().unwrap_or_default()
        }
    }
}

impl Default for DictionaryData {
    fn default() -> Self {
        Self::new()
    }
}

/// Java `org.omegat.core.dictionaries.DictionariesManager` ignore list + lookup.
#[derive(Debug, Default)]
pub struct DictionariesManager {
    pub ignore: std::collections::HashSet<String>,
}

impl DictionariesManager {
    pub fn add_ignore_word(&mut self, word: &str) {
        self.ignore.insert(word.to_lowercase());
    }

    pub fn load_ignore_words(&mut self, path: &Path) {
        if let Ok(raw) = std::fs::read_to_string(path) {
            for line in raw.lines() {
                let w = line.trim();
                if !w.is_empty() {
                    self.ignore.insert(w.to_lowercase());
                }
            }
        }
    }

    pub fn is_ignored(&self, word: &str) -> bool {
        self.ignore.contains(&word.to_lowercase())
    }

    pub fn find_words(&self, dict_dir: &Path, words: &[&str]) -> Vec<DictHitDto> {
        let mut out = Vec::new();
        for w in words {
            if self.is_ignored(w) {
                continue;
            }
            out.extend(lookup(dict_dir, w));
        }
        out
    }
}

fn read_maybe_gzip(path: &Path) -> Option<String> {
    read_maybe_gzip_bytes(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn read_maybe_gzip_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    if path.extension().and_then(|e| e.to_str()) == Some("dz")
        || path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with(".dz"))
        || bytes.starts_with(&[0x1f, 0x8b])
    {
        let mut dec = GzDecoder::new(bytes.as_slice());
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;
        return Ok(out);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn fixture_dsl_lookup() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dict");
        let hits = lookup(&dir, "omega");
        assert_eq!(hits[0].word, "omega");
        assert!(hits[0].definition.contains("translation"));
        let fuzzy = lookup_opts(&dir, "omegx", true);
        assert!(fuzzy.iter().any(|h| h.word == "omega"), "{fuzzy:?}");
    }

    #[test]
    fn dsl_lookup() {
        let raw = "hello\n  a greeting\nworld\n  the earth\n";
        let hits = parse_dsl(raw, "hel", "t.dsl");
        assert_eq!(hits[0].word, "hello");
        assert!(hits[0].definition.contains("greeting"));
    }

    #[test]
    fn stardict_idx() {
        let mut idx = Vec::new();
        idx.extend(b"cat\0");
        idx.extend(0u32.to_be_bytes());
        idx.extend(3u32.to_be_bytes());
        let dict = b"felidae extra";
        let hits = parse_stardict_idx(&idx, dict, "cat", "x.ifo");
        assert_eq!(hits[0].definition, "fel");
    }

    #[test]
    fn dsl_dz_and_dict_dz() {
        let dir = tempdir().unwrap();
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"omega\n  CAT tool\n").unwrap();
        std::fs::write(dir.path().join("demo.dsl.dz"), enc.finish().unwrap()).unwrap();
        let hits = lookup(dir.path(), "omega");
        assert_eq!(hits[0].word, "omega");

        let stem = dir.path().join("sd");
        std::fs::write(stem.with_extension("ifo"), "StarDict\n").unwrap();
        let mut idx = Vec::new();
        idx.extend(b"omega\0");
        idx.extend(0u32.to_be_bytes());
        idx.extend(3u32.to_be_bytes());
        std::fs::write(stem.with_extension("idx"), idx).unwrap();
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"CAT").unwrap();
        std::fs::write(format!("{}.dict.dz", stem.display()), enc.finish().unwrap()).unwrap();
        let hits = lookup(dir.path(), "omega");
        assert!(hits.iter().any(|h| h.definition == "CAT"), "{hits:?}");
    }
}
