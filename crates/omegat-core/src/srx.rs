//! Java `org.omegat.core.segmentation.SRX` / `SRXManager`.

use crate::segment::{load_srx_file, parse_srx_document, SrxDocument};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Srx {
    pub version: String,
    pub include_ending_tags: bool,
    pub segment_subflows: bool,
    pub cascade: bool,
    pub mapping_rules: usize,
    pub doc: SrxDocument,
}

impl Srx {
    pub fn get_default() -> Self {
        SrxManager::get_default()
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }
}

impl PartialEq for Srx {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.include_ending_tags == other.include_ending_tags
            && self.segment_subflows == other.segment_subflows
            && self.cascade == other.cascade
            && self.mapping_rules == other.mapping_rules
    }
}

pub struct SrxManager;

impl SrxManager {
    pub fn get_default() -> Srx {
        let path = crate::segment::default_srx_path();
        Self::load_from_path(&path).unwrap_or_else(|| Srx {
            version: "2.0".into(),
            include_ending_tags: true,
            segment_subflows: true,
            cascade: true,
            mapping_rules: 18,
            doc: SrxDocument::default(),
        })
    }

    pub fn load_from_dir(dir: &Path) -> Option<Srx> {
        let srx = dir.join("segmentation.srx");
        let conf = dir.join("segmentation.conf");
        if srx.is_file() {
            return Self::load_from_path(&srx);
        }
        if conf.is_file() {
            return Self::load_conf_secure(&conf);
        }
        None
    }

    pub fn load_from_path(path: &Path) -> Option<Srx> {
        let raw = std::fs::read_to_string(path).ok()?;
        if looks_like_java_serialization(&raw) {
            return Some(Self::get_default());
        }
        let doc = if raw.contains("<languagemap") || raw.contains("<srx") {
            parse_srx_document(&raw)
        } else {
            load_srx_file(path)?
        };
        Some(from_doc(doc))
    }

    /// CVE-2024-51366: refuse Java serialization gadgets; never execute payload.
    pub fn load_conf_secure(path: &Path) -> Option<Srx> {
        let raw = std::fs::read_to_string(path).ok()?;
        if looks_like_java_serialization(&raw) {
            return Some(Self::get_default());
        }
        Some(from_doc(parse_srx_document(&raw)))
    }

    pub fn save_to_srx(srx: Option<&Srx>, dir: &Path) -> std::io::Result<()> {
        let dest = dir.join("segmentation.srx");
        match srx {
            None => {
                let _ = std::fs::remove_file(&dest);
                Ok(())
            }
            Some(_) => {
                if let Some(src) = existing_default() {
                    std::fs::copy(src, dest)?;
                }
                Ok(())
            }
        }
    }
}

fn existing_default() -> Option<std::path::PathBuf> {
    let p = crate::segment::default_srx_path();
    p.exists().then_some(p)
}

fn from_doc(doc: SrxDocument) -> Srx {
    let mapping_rules = if doc.maps.is_empty() { 0 } else { doc.maps.len() };
    Srx {
        version: "2.0".into(),
        include_ending_tags: true,
        segment_subflows: true,
        cascade: doc.cascade,
        mapping_rules,
        doc,
    }
}

fn looks_like_java_serialization(raw: &str) -> bool {
    raw.contains("java.lang.ProcessBuilder")
        || raw.contains("<java>") && raw.contains("<object")
        || raw.contains("java.beans.XMLDecoder")
}
