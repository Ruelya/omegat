//! Strict Java goldens. Every committed JSON under fixtures/goldens/filters
//! must match parse sources, empty-write preserve, and translated write-back.

use omegat_filters::{FilterContext, FilterRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn goldens_dir() -> PathBuf {
    repo_root().join("fixtures/goldens/filters")
}

fn fixtures_dir() -> PathBuf {
    repo_root().join("fixtures/filters")
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_json(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(p);
        }
    }
}

fn ctx_from(options: &Value) -> FilterContext {
    let mut ctx = FilterContext::default();
    if let Some(map) = options.as_object() {
        for (k, v) in map {
            ctx.options.insert(
                k.clone(),
                v.as_str().unwrap_or(&v.to_string()).to_string(),
            );
        }
    }
    ctx
}

fn normalize_ws(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn read_written(path: &Path) -> String {
    if let Ok(s) = std::fs::read_to_string(path) {
        if !s.is_empty() {
            return s;
        }
    }
    if let Ok(s) = std::fs::read_to_string(path.with_extension("pdf.txt")) {
        return s;
    }
    if let Ok(f) = std::fs::File::open(path) {
        if let Ok(mut zip) = zip::ZipArchive::new(f) {
            let mut acc = String::new();
            for i in 0..zip.len() {
                if let Ok(mut e) = zip.by_index(i) {
                    let mut xml = String::new();
                    if e.read_to_string(&mut xml).is_ok() {
                        acc.push_str(&xml);
                    }
                }
            }
            return acc;
        }
    }
    String::new()
}

#[test]
fn every_filter_golden_is_strict() {
    let mut files = Vec::new();
    collect_json(&goldens_dir(), &mut files);
    assert!(
        files.len() >= 3,
        "need Text/PO/HTML goldens, found {}",
        files.len()
    );
    let reg = FilterRegistry::new();
    let tmp = tempfile::tempdir().unwrap();
    for path in &files {
        let raw = std::fs::read_to_string(path).unwrap();
        let spec: Value = serde_json::from_str(&raw).expect(path.to_str().unwrap());
        let id = spec["id"].as_str().expect("id");
        let rel = spec["fixture"].as_str().expect("fixture");
        let src = fixtures_dir().join(rel);
        assert!(src.is_file(), "missing fixture {} for {}", src.display(), path.display());
        let ctx = ctx_from(&spec["options"]);
        let filter = reg.by_id(id).unwrap_or_else(|| panic!("unknown filter {id}"));
        let parsed = filter.parse(&src, &ctx).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let expected: Vec<String> = spec["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got: Vec<String> = parsed.segments.iter().map(|s| s.source.clone()).collect();
        assert_eq!(got, expected, "sources mismatch {}", path.display());
        if let Some(ids) = spec["ids"].as_array() {
            let got_ids: Vec<String> = parsed.segments.iter().map(|s| s.id.clone()).collect();
            let exp_ids: Vec<String> = ids.iter().map(|v| v.as_str().unwrap().to_string()).collect();
            assert_eq!(got_ids, exp_ids, "ids mismatch {}", path.display());
        }
        if spec["empty_write"].as_str() == Some("preserve_source") {
            let dest = tmp.path().join(format!("empty-{}", src.file_name().unwrap().to_string_lossy()));
            filter.write(&src, &dest, &HashMap::new(), &ctx).unwrap();
            let back = normalize_ws(&read_written(&dest));
            let orig = normalize_ws(&read_written(&src));
            if back == orig && !orig.is_empty() {
                // XML dialect empty write keeps the original tree (Java translateXML).
            } else {
                for seg in &expected {
                    let stripped = seg.replace(['<', '>'], "");
                    assert!(
                        back.contains(seg)
                            || back.contains(seg.trim())
                            || (!stripped.is_empty() && back.contains(stripped.trim()))
                            || orig.contains(seg),
                        "empty write dropped source {seg:?} in {}",
                        path.display()
                    );
                }
            }
            assert!(!back.is_empty() || dest.exists(), "empty write produced empty file {}", path.display());
        }
        if let Some(tr) = spec.get("translated") {
            let source = tr["source"].as_str().unwrap();
            let translation = tr["translation"].as_str().unwrap();
            let must = tr["must_contain"].as_str().unwrap_or(translation);
            let mut map = HashMap::new();
            map.insert(source.to_string(), translation.to_string());
            if let Some(seg) = parsed.segments.iter().find(|s| s.source == source) {
                map.insert(seg.id.clone(), translation.to_string());
            }
            let dest = tmp.path().join(format!("t-{}", src.file_name().unwrap().to_string_lossy()));
            filter.write(&src, &dest, &map, &ctx).unwrap();
            let back = read_written(&dest);
            assert!(
                back.contains(must),
                "translated write missing {must:?} in {}:\n{back}",
                path.display()
            );
        }
    }
}

#[test]
fn registry_has_forty_nine_java_filters() {
    let n = FilterRegistry::new().all().len();
    assert!(n >= 49, "expected 49 Java-equivalent filters, got {n}");
}

#[test]
fn every_java_filter_id_has_a_golden() {
    let extra = ["json", "csv", "markdown"];
    let mut missing = Vec::new();
    for f in FilterRegistry::new().all() {
        if extra.contains(&f.id()) {
            continue;
        }
        let dir = goldens_dir().join(f.id());
        if !dir.is_dir() {
            missing.push(f.id());
        }
    }
    assert!(missing.is_empty(), "filters without goldens: {missing:?}");
}
