use omegat_ipc::DictHitDto;
use std::path::Path;

/// StarDict (.ifo/.idx/.dict) and Lingvo DSL (.dsl / .dsl.dz as UTF-8 text).
pub fn lookup(dir: &Path, word: &str) -> Vec<DictHitDto> {
    if !dir.exists() || word.is_empty() {
        return vec![];
    }
    let mut hits = Vec::new();
    let needle = word.to_lowercase();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return hits;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if name.ends_with(".dsl") || name.ends_with(".txt") {
            hits.extend(lookup_dsl(&p, &needle));
        } else if name.ends_with(".ifo") {
            hits.extend(lookup_stardict(&p, &needle));
        }
    }
    hits
}

fn lookup_dsl(path: &Path, needle: &str) -> Vec<DictHitDto> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vec![];
    };
    parse_dsl(&raw, needle, &path.display().to_string())
}

pub fn parse_dsl(raw: &str, needle: &str, source: &str) -> Vec<DictHitDto> {
    let mut hits = Vec::new();
    let mut head = String::new();
    let mut def = String::new();
    let flush = |head: &mut String, def: &mut String, hits: &mut Vec<DictHitDto>| {
        if !head.is_empty() && head.to_lowercase().contains(needle) {
            hits.push(DictHitDto {
                word: head.clone(),
                definition: def.trim().to_string(),
                source: source.to_string(),
            });
        }
        head.clear();
        def.clear();
    };
    for line in raw.lines() {
        if line.starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t']) && !line.is_empty() {
            flush(&mut head, &mut def, &mut hits);
            head = line.trim().to_string();
        } else {
            def.push_str(line.trim());
            def.push('\n');
        }
    }
    flush(&mut head, &mut def, &mut hits);
    hits
}

fn lookup_stardict(ifo: &Path, needle: &str) -> Vec<DictHitDto> {
    let stem = ifo.with_extension("");
    let idx = stem.with_extension("idx");
    let dict = if stem.with_extension("dict").exists() {
        stem.with_extension("dict")
    } else {
        return vec![];
    };
    let Ok(idx_bytes) = std::fs::read(&idx) else {
        return vec![];
    };
    let Ok(dict_bytes) = std::fs::read(&dict) else {
        return vec![];
    };
    parse_stardict_idx(&idx_bytes, &dict_bytes, needle, &ifo.display().to_string())
}

/// StarDict idx: utf-8 word, NUL, 32-bit offset, 32-bit size (big-endian).
pub fn parse_stardict_idx(idx: &[u8], dict: &[u8], needle: &str, source: &str) -> Vec<DictHitDto> {
    let mut hits = Vec::new();
    let mut i = 0;
    while i < idx.len() {
        let Some(z) = idx[i..].iter().position(|&b| b == 0) else { break };
        let word = String::from_utf8_lossy(&idx[i..i + z]).into_owned();
        i += z + 1;
        if i + 8 > idx.len() {
            break;
        }
        let off = u32::from_be_bytes(idx[i..i + 4].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(idx[i + 4..i + 8].try_into().unwrap()) as usize;
        i += 8;
        if word.to_lowercase().contains(needle) && off + size <= dict.len() {
            hits.push(DictHitDto {
                word,
                definition: String::from_utf8_lossy(&dict[off..off + size]).into_owned(),
                source: source.to_string(),
            });
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
