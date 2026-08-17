//! External TM folders: `tm/auto`, `tm/enforce`, `tm/mt`, `tm/penalty-NNN`, `tmx2source`.

use crate::consts::*;
use crate::properties::ProjectProperties;
use crate::tmx::{ProjectTmx, TmxEntry};

pub fn penalty_from_origin(origin: &str) -> i32 {
    origin
        .replace('\\', "/")
        .split('/')
        .find_map(|p| p.strip_prefix("penalty-")?.parse().ok())
        .unwrap_or(0)
}

pub fn folder_is(origin: &str, name: &str) -> bool {
    let o = origin.replace('\\', "/");
    o == name || o.starts_with(&format!("{name}/")) || o.contains(&format!("/{name}/"))
}

pub fn load_external_tm(props: &ProjectProperties) -> Vec<(TmxEntry, String)> {
    let mut out = Vec::new();
    if !props.tm_dir.exists() {
        return out;
    }
    for ent in walkdir::WalkDir::new(&props.tm_dir).into_iter().flatten() {
        if ent.path().extension().and_then(|e| e.to_str()) != Some("tmx") {
            continue;
        }
        let origin = ent
            .path()
            .strip_prefix(&props.tm_dir)
            .unwrap_or(ent.path())
            .to_string_lossy()
            .replace('\\', "/");
        let langs = if folder_is(&origin, TMX2SOURCE) {
            let stem = ent
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&props.target_lang);
            (props.source_lang.as_str(), stem)
        } else {
            (props.source_lang.as_str(), props.target_lang.as_str())
        };
        if let Ok(tmx) = ProjectTmx::load(ent.path(), langs.0, langs.1) {
            let penalty = penalty_from_origin(&origin);
            for mut e in tmx.entries {
                e.penalty = penalty;
                if penalty > 0 {
                    e.note = Some(format!("penalty:{penalty}"));
                }
                out.push((e, origin.clone()));
            }
        }
    }
    out
}
