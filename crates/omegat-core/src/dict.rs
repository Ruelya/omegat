use omegat_ipc::DictHitDto;
use std::path::Path;

/// StarDict (.ifo/.dict) and Lingvo DSL readers — structural subset.
pub fn lookup(dir: &Path, word: &str) -> Vec<DictHitDto> {
    if !dir.exists() {
        return vec![];
    }
    let mut hits = Vec::new();
    let needle = word.to_lowercase();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "dsl" | "txt" | "dict" | "ifo") {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    for line in raw.lines() {
                        if line.to_lowercase().contains(&needle) && !line.trim().is_empty() {
                            hits.push(DictHitDto {
                                word: word.to_string(),
                                definition: line.trim().to_string(),
                                source: path.file_name().unwrap_or_default().to_string_lossy().into(),
                            });
                            if hits.len() >= 8 {
                                return hits;
                            }
                        }
                    }
                }
            }
        }
    }
    hits
}
