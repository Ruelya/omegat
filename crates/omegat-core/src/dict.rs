use flate2::read::GzDecoder;
use omegat_ipc::DictHitDto;
use std::io::Read;
use std::path::Path;

/// StarDict (`.ifo` + `.idx` + `.dict` / `.dict.dz`) and Lingvo DSL (`.dsl` / `.dsl.dz`).
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
        if name.ends_with(".dsl") || name.ends_with(".dsl.dz") {
            hits.extend(lookup_dsl(&p, &needle));
        } else if name.ends_with(".ifo") {
            hits.extend(lookup_stardict(&p, &needle));
        }
    }
    hits
}

fn lookup_dsl(path: &Path, needle: &str) -> Vec<DictHitDto> {
    let raw = read_maybe_gzip(path).unwrap_or_default();
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
    let Some(dict) = stardict_dict_path(&stem) else {
        return vec![];
    };
    let Ok(idx_bytes) = std::fs::read(&idx) else {
        return vec![];
    };
    let Ok(dict_bytes) = read_maybe_gzip_bytes(&dict) else {
        return vec![];
    };
    parse_stardict_idx(&idx_bytes, &dict_bytes, needle, &ifo.display().to_string())
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

fn read_maybe_gzip(path: &Path) -> Option<String> {
    read_maybe_gzip_bytes(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn read_maybe_gzip_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    if path.extension().and_then(|e| e.to_str()) == Some("dz")
        || path.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.ends_with(".dz"))
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
