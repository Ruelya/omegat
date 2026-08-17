use omegat_ipc::GlossaryHitDto;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
    pub comment: String,
}

pub fn load_glossary(path: &Path) -> Vec<GlossaryEntry> {
    if !path.exists() {
        return vec![];
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vec![];
    };
    parse_glossary(&raw)
}

pub fn parse_glossary(raw: &str) -> Vec<GlossaryEntry> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('<') && line.contains("<term") {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            out.push(GlossaryEntry {
                source: parts[0].trim().to_string(),
                target: parts[1].trim().to_string(),
                comment: parts.get(2).unwrap_or(&"").trim().to_string(),
            });
        }
    }
    // TBX-ish: <term>text</term> pairs
    if out.is_empty() && raw.contains("<term") {
        let mut terms = Vec::new();
        let mut rest = raw;
        while let Some(s) = rest.find("<term") {
            let after = &rest[s..];
            if let Some(gt) = after.find('>') {
                if let Some(end) = after[gt + 1..].find("</term>") {
                    terms.push(after[gt + 1..gt + 1 + end].to_string());
                    rest = &after[gt + 1 + end + 7..];
                    continue;
                }
            }
            break;
        }
        for pair in terms.chunks(2) {
            if pair.len() == 2 {
                out.push(GlossaryEntry {
                    source: pair[0].clone(),
                    target: pair[1].clone(),
                    comment: "tbx".into(),
                });
            }
        }
    }
    out
}

pub fn lookup(entries: &[GlossaryEntry], segment: &str) -> Vec<GlossaryHitDto> {
    let lower = segment.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            !e.source.is_empty() && lower.contains(&e.source.to_lowercase())
        })
        .map(|e| GlossaryHitDto {
            source: e.source.clone(),
            target: e.target.clone(),
            comment: e.comment.clone(),
        })
        .collect()
}

pub fn append_entry(path: &Path, source: &str, target: &str, comment: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = format!("{source}\t{target}");
    if !comment.is_empty() {
        line.push('\t');
        line.push_str(comment);
    }
    line.push('\n');
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(line.as_bytes())
}
