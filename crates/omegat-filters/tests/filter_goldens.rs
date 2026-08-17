//! Java-exported goldens. Segment lists and write-back are `assert_eq`.
//! Missing or divergent implementations must fail. Red is allowed.

use omegat_filters::{FilterContext, FilterRegistry};
use serde_json::Value;
use std::collections::HashMap;
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
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            if p.file_name().and_then(|n| n.to_str()) == Some("_voided") {
                continue;
            }
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

fn valid_java_test(s: &str) -> bool {
    let Some((cls, method)) = s.split_once('#') else {
        return false;
    };
    cls.starts_with("org.omegat.")
        && cls
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        && method
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !method.is_empty()
}

#[test]
fn g0_text_po_html_goldens_exist() {
    for rel in [
        "text/file-TextFilter.empty-lines.json",
        "po/file-POFilter-multiple.json",
        "html/file-HTMLFilter2.json",
    ] {
        let p = goldens_dir().join(rel);
        assert!(p.is_file(), "missing Java-exported golden {}", p.display());
    }
}

fn is_g1_filter_golden(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/");
    s.ends_with("/text/file-TextFilter.empty-lines.json")
        || s.ends_with("/po/file-POFilter-multiple.json")
        || s.ends_with("/html/file-HTMLFilter2.json")
}

fn check_provenance(path: &Path, spec: &Value) {
    let java_test = spec["java_test"].as_str().unwrap_or("");
    assert!(
        valid_java_test(java_test),
        "fake or missing java_test in {}: {java_test:?}",
        path.display()
    );
    assert_eq!(
        spec["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens"),
        "not a Java export: {}",
        path.display()
    );
    assert!(
        spec.get("must_contain").is_none(),
        "must_contain is forbidden in {}",
        path.display()
    );
    if let Some(tr) = spec.get("translated") {
        assert!(
            tr.get("must_contain").is_none(),
            "translated.must_contain is forbidden in {}",
            path.display()
        );
    }
}

fn assert_filter_golden(path: &Path, spec: &Value, tmp: &Path) {
    let reg = FilterRegistry::new();
    let id = spec["id"].as_str().expect("id");
    let rel = spec["fixture"].as_str().expect("fixture");
    let src = fixtures_dir().join(rel);
    assert!(
        src.is_file(),
        "missing fixture {} for {}",
        src.display(),
        path.display()
    );
    let ctx = ctx_from(&spec["options"]);
    let filter = reg
        .by_id(id)
        .unwrap_or_else(|| panic!("unknown filter {id}"));
    let parsed = filter
        .parse(&src, &ctx)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
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
        let exp_ids: Vec<String> = ids
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        if exp_ids.iter().any(|s| !s.is_empty()) {
            assert_eq!(got_ids, exp_ids, "ids mismatch {}", path.display());
        }
    }
    if let Some(empty_text) = spec["empty_write_text"].as_str() {
        let dest = tmp.join(format!(
            "empty-{}",
            src.file_name().unwrap().to_string_lossy()
        ));
        filter
            .write(&src, &dest, &HashMap::new(), &ctx)
            .unwrap_or_else(|e| panic!("empty write {}: {e}", path.display()));
        let back = normalize_ws(&std::fs::read_to_string(&dest).unwrap_or_default());
        let exp = normalize_ws(empty_text);
        assert_eq!(back, exp, "empty write mismatch {}", path.display());
    }
    if let Some(tr) = spec.get("translated") {
        let source = tr["source"].as_str().unwrap();
        let translation = tr["translation"].as_str().unwrap();
        let mut map = HashMap::new();
        map.insert(source.to_string(), translation.to_string());
        if let Some(seg) = parsed.segments.iter().find(|s| s.source == source) {
            map.insert(seg.id.clone(), translation.to_string());
        }
        let dest = tmp.join(format!("t-{}", src.file_name().unwrap().to_string_lossy()));
        filter
            .write(&src, &dest, &map, &ctx)
            .unwrap_or_else(|e| panic!("translated write {}: {e}", path.display()));
        let back = normalize_ws(&std::fs::read_to_string(&dest).unwrap_or_default());
        if let Some(exp) = spec["translated_write"].as_str() {
            assert_eq!(
                back,
                normalize_ws(exp),
                "translated write mismatch {}",
                path.display()
            );
        } else {
            panic!(
                "translated_write missing in {} (contains/must_contain are forbidden)",
                path.display()
            );
        }
    }
}

fn assert_rel(rel: &str, tmp: &Path) {
    let path = goldens_dir().join(rel);
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    check_provenance(&path, &spec);
    assert_filter_golden(&path, &spec, tmp);
}

/// G1 gate: Text / PO / HTML only.
#[test]
fn g1_text_po_html_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "text/file-TextFilter.empty-lines.json",
        "po/file-POFilter-multiple.json",
        "html/file-HTMLFilter2.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g2_ini_srt_yaml_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "ini/file-INIFilter.json",
        "srt/file-SrtFilter.json",
        "yaml/sample1.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g2_hhc_dokuwiki_sbv_vtt_xtag_latex_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "hhc/file-HHCFilter2.json",
        "dokuwiki/dokuwiki.json",
        "sbv/simple.json",
        "webvtt/simple.json",
        "xtag/file-XtagFilter.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g2_latex_pdf_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in ["latex/file-latex-items.json", "pdf/file-PdfFilter.json"] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g2_properties_dtd_php_lang_ftl_csv_ilias_rc_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "properties/file-ResourceBundleFilter.json",
        "mozdtd/file.json",
        "moodlephp/file.json",
        "mozlang/file-MozillaLangFilter-de.json",
        "mozftl/MozillaFTLFilter.json",
        "magento/MagentoFilter.json",
        "ilias/ILIASFilter.json",
        "rc/prog.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}


#[test]
fn committed_filter_goldens_have_java_provenance() {
    let mut files = Vec::new();
    collect_json(&goldens_dir(), &mut files);
    assert!(
        files.iter().any(|p| is_g1_filter_golden(p)),
        "Text/PO/HTML Java goldens missing under fixtures/goldens/filters"
    );
    for path in &files {
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        check_provenance(path, &spec);
    }
}
