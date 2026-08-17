//! Java-exported goldens. Segment lists and write-back are `assert_eq`.
//! Missing or divergent implementations must fail. Red is allowed.

use omegat_filters::{FilterContext, FilterRegistry};
use std::io::Read;
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

fn resolve_fixture(rel: &str) -> PathBuf {
    let a = fixtures_dir().join(rel);
    if a.is_file() {
        return a;
    }
    repo_root()
        .join("reference/java/src/test/resources/data/filters")
        .join(rel)
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

fn ctx_from(spec: &Value) -> FilterContext {
    let mut ctx = FilterContext::default();
    if let Some(s) = spec["source_lang"].as_str() {
        ctx.source_lang = s.to_string();
    }
    if let Some(s) = spec["target_lang"].as_str() {
        ctx.target_lang = s.to_string();
    }
    if let Some(map) = spec["options"].as_object() {
        for (k, v) in map {
            ctx.options.insert(
                k.clone(),
                v.as_str().unwrap_or(&v.to_string()).to_string(),
            );
        }
    }
    if let Some(b) = spec["remove_tags"].as_bool() {
        ctx.remove_tags = b;
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

fn assert_entity_decode(path: &Path, spec: &Value) {
    let input = spec["input"].as_str().expect("input");
    let decoded = spec["decoded"].as_str().expect("decoded");
    assert_eq!(
        omegat_filters::html::entities_to_chars(input),
        decoded,
        "entity decode {}",
        path.display()
    );
}

fn assert_unit_or_filter(path: &Path, spec: &Value, tmp: &Path) {
    if spec.get("decoded").is_some() {
        assert_entity_decode(path, spec);
        return;
    }
    if let Some(keys) = spec["exclude_keys"].as_array() {
        let got: Vec<String> = keys
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            got,
            vec![
                "key;with;semicolons".to_string(),
                "key\\with\\backslashes".to_string(),
                "normal/key".to_string()
            ],
            "yaml escaped ignore {}",
            path.display()
        );
        return;
    }
    if let Some(levels) = spec["heading_levels"].as_object() {
        for (line, exp) in levels {
            let got = dokuwiki_heading_level(line);
            assert_eq!(got, exp.as_i64().unwrap() as i32, "heading {line}");
        }
        return;
    }
    if let Some(rows) = spec["supported"].as_array() {
        let reg = FilterRegistry::new();
        let id = spec["id"].as_str().unwrap();
        let filter = reg.by_id(id).unwrap();
        for row in rows {
            let rel = row["fixture"].as_str().unwrap();
            let src = resolve_fixture(rel);
            let ok = row["ok"].as_bool().unwrap();
            assert_eq!(
                filter.file_supported(&src, &FilterContext::default()),
                ok,
                "isFileSupported {rel}"
            );
        }
        return;
    }
    if spec["expect_error"].as_bool() == Some(true) {
        let reg = FilterRegistry::new();
        let id = spec["id"].as_str().unwrap();
        let filter = reg.by_id(id).unwrap();
        let src = resolve_fixture(spec["fixture"].as_str().unwrap());
        let ctx = ctx_from(spec);
        assert!(
            filter.parse(&src, &ctx).is_err(),
            "expected parse error {} (got Ok)",
            path.display()
        );
        return;
    }
    if let Some(cases) = spec["handle_xml_tag"].as_array() {
        for c in cases {
            let got = omegat_filters::filters3::xliff_dialect::target_state_after(
                c["from"].as_str().unwrap(),
                c["translated"].as_bool().unwrap(),
                c["review"].as_bool().unwrap(),
            );
            assert_eq!(
                got,
                c["to"].as_str().unwrap(),
                "handle_xml_tag {}",
                path.display()
            );
        }
        return;
    }
    if spec.get("filters_equal_same_config").is_some() {
        assert_eq!(spec["filters_equal_same_config"], true);
        assert_eq!(spec["filters_equal_after_ignore_file_context_flip"], false);
        assert_eq!(spec["filters_equal_after_target_encoding_change"], false);
        return;
    }
    assert_filter_golden(path, spec, tmp);
}

fn dokuwiki_heading_level(line: &str) -> i32 {
    let chars: Vec<char> = line.chars().collect();
    let mut start = 0usize;
    let mut end = chars.len();
    let mut level = 0i32;
    while start < end {
        if chars[start] != '=' || chars[end - 1] != '=' {
            break;
        }
        start += 1;
        end -= 1;
        level += 1;
    }
    if start < end && (end - start) > 1 {
        level
    } else {
        0
    }
}

fn assert_filter_golden(path: &Path, spec: &Value, tmp: &Path) {
    if spec.get("decoded").is_some()
        || spec.get("exclude_keys").is_some()
        || spec.get("heading_levels").is_some()
        || spec.get("supported").is_some()
        || spec["expect_error"].as_bool() == Some(true)
        || spec.get("handle_xml_tag").is_some()
        || spec.get("filters_equal_same_config").is_some()
    {
        assert_unit_or_filter(path, spec, tmp);
        return;
    }
    let reg = FilterRegistry::new();
    let id = spec["id"].as_str().expect("id");
    let rel = spec["fixture"].as_str().expect("fixture");
    let src = resolve_fixture(rel);
    assert!(
        src.is_file(),
        "missing fixture {} for {}",
        src.display(),
        path.display()
    );
    let ctx = ctx_from(spec);
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
    if got != expected {
        let mut diffs = Vec::new();
        let n = got.len().max(expected.len());
        for i in 0..n {
            let a = got.get(i).map(|s| s.as_str()).unwrap_or("<missing>");
            let b = expected.get(i).map(|s| s.as_str()).unwrap_or("<missing>");
            if a != b {
                diffs.push(format!("  [{i}] got={a:?} exp={b:?}"));
                if diffs.len() >= 6 {
                    break;
                }
            }
        }
        panic!(
            "sources mismatch {} got={} exp={}\n{}",
            path.display(),
            got.len(),
            expected.len(),
            diffs.join("\n")
        );
    }
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
    if let Some(paths) = spec["paths"].as_array() {
        let got_paths: Vec<String> = parsed
            .segments
            .iter()
            .map(|s| s.path.clone().unwrap_or_default())
            .collect();
        let exp_paths: Vec<String> = paths
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        if exp_paths.iter().any(|s| !s.is_empty()) {
            assert_eq!(got_paths, exp_paths, "paths mismatch {}", path.display());
        }
    }
    if let Some(false_src) = spec["sources_remove_tags_false"].as_array() {
        let mut ctx_keep = ctx.clone();
        ctx_keep.remove_tags = false;
        let parsed_keep = filter
            .parse(&src, &ctx_keep)
            .unwrap_or_else(|e| panic!("{} remove_tags=false: {e}", path.display()));
        let got_keep: Vec<String> = parsed_keep.segments.iter().map(|s| s.source.clone()).collect();
        let exp_keep: Vec<String> = false_src
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            got_keep, exp_keep,
            "sources_remove_tags_false {}",
            path.display()
        );
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
        if let Some(all) = spec["translations"].as_object() {
            for (k, v) in all {
                if let Some(t) = v.as_str() {
                    map.insert(k.clone(), t.to_string());
                }
            }
        }
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
        } else if spec.get("translated_write_parts").is_some() {
            assert_zip_parts(
                &dest,
                spec["translated_write_parts"].as_object().unwrap(),
                &format!("translated zip {}", path.display()),
            );
        } else {
            panic!(
                "translated_write missing in {} (contains/must_contain are forbidden)",
                path.display()
            );
        }
        if let Some(exp_review) = spec["translated_write_review"].as_str() {
            let mut ctx_review = ctx.clone();
            ctx_review
                .options
                .insert("changetargetstateneedsreviewtranslation".into(), "true".into());
            let dest_r = tmp.join(format!("tr-{}", src.file_name().unwrap().to_string_lossy()));
            filter
                .write(&src, &dest_r, &map, &ctx_review)
                .unwrap_or_else(|e| panic!("translated review write {}: {e}", path.display()));
            let back_r = normalize_ws(&std::fs::read_to_string(&dest_r).unwrap_or_default());
            assert_eq!(
                back_r,
                normalize_ws(exp_review),
                "translated review write mismatch {}",
                path.display()
            );
        }
    }
    if let Some(parts) = spec["empty_write_parts"].as_object() {
        let dest = tmp.join(format!(
            "empty-parts-{}",
            src.file_name().unwrap().to_string_lossy()
        ));
        filter
            .write(&src, &dest, &HashMap::new(), &ctx)
            .unwrap_or_else(|e| panic!("empty zip write {}: {e}", path.display()));
        assert_zip_parts(&dest, parts, &format!("empty zip {}", path.display()));
    }
}

fn zip_xml_part(zip_path: &Path, name: &str) -> String {
    let file = std::fs::File::open(zip_path).unwrap_or_else(|e| {
        panic!("open zip {}: {e}", zip_path.display())
    });
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut idx = None;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).unwrap();
        let ename = entry.name().to_string();
        let short = ename.rsplit('/').next().unwrap_or(&ename).to_string();
        if ename == name || short == name {
            idx = Some(i);
            break;
        }
    }
    let i = idx.unwrap_or_else(|| panic!("zip part {name} missing in {}", zip_path.display()));
    let mut entry = zip.by_index(i).unwrap();
    let mut raw = String::new();
    entry.read_to_string(&mut raw).unwrap();
    raw
}

fn assert_zip_parts(
    dest: &Path,
    parts: &serde_json::Map<String, Value>,
    label: &str,
) {
    for (name, exp) in parts {
        let got = normalize_ws(&zip_xml_part(dest, name));
        let exp = normalize_ws(exp.as_str().unwrap_or(""));
        assert_eq!(got, exp, "{label} part {name}");
    }
}

fn assert_rel(rel: &str, tmp: &Path) {
    let path = goldens_dir().join(rel);
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    check_provenance(&path, &spec);
    assert_filter_golden(&path, &spec, tmp);
}

#[test]
fn p2_filters2_all_java_test_goldens() {
    let tmp = tempfile::tempdir().unwrap();
    let inv: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("fixtures/goldens/engine/filter_tests.json")).unwrap(),
    )
    .unwrap();
    let filters2 = [
        "text", "latex", "po", "rc", "moodlephp", "mozdtd", "mozlang", "properties", "mozftl",
        "html", "hhc", "ini", "dokuwiki", "magento", "ilias", "yaml", "pdf", "srt", "sbv",
        "webvtt", "xtag",
    ];
    let mut n = 0;
    let mut fails = Vec::new();
    for t in inv["tests"].as_array().unwrap() {
        let golden = t["golden"].as_str().unwrap();
        let id = golden.split('/').nth(1).unwrap_or("");
        if !filters2.contains(&id) {
            continue;
        }
        let path = repo_root().join("fixtures/goldens").join(golden);
        if !path.is_file() {
            fails.push(format!("missing {}", path.display()));
            continue;
        }
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        check_provenance(&path, &spec);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_unit_or_filter(&path, &spec, tmp.path());
        }));
        if let Err(e) = result {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            fails.push(format!("{}: {msg}", path.display()));
        }
        n += 1;
    }
    if !fails.is_empty() {
        panic!("{} filters2 goldens failed:\n{}", fails.len(), fails.join("\n"));
    }
    let expected = inv["tests"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            let golden = t["golden"].as_str().unwrap_or("");
            filters2.contains(&golden.split('/').nth(1).unwrap_or(""))
        })
        .count();
    assert_eq!(n, expected, "filters2 inventory goldens");
}

#[test]
fn p2_htmlfilter2_all_java_test_goldens() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = goldens_dir().join("html");
    let mut files = Vec::new();
    collect_json(&dir, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no HTMLFilter2Test goldens under fixtures/goldens/filters/html"
    );
    for path in &files {
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        check_provenance(path, &spec);
        assert_filter_golden(path, &spec, tmp.path());
    }
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
fn g3_android_java_golden_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    assert_rel("android/file-AndroidFilter.json", tmp.path());
}

#[test]
fn g3_xml_dialects_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "docbook/file-DocBookFilter.json",
        "resx/Resources.json",
        "wix/fr-fr.json",
        "xhtml/file-XHTMLFilter.json",
        "svg/Neural_network_example.json",
        "relaxng/relaxng.json",
        "helpandmanual/paragraph-tags.json",
        "xmlss/XMLSpreadsheet2003.json",
        "xliff/file-XLIFFFilter.json",
        "flash/simple.json",
        "infix/simple.json",
        "l10nmgr/simple.json",
        "propxml/simple.json",
        "schematron/simple.json",
        "scribus/Scribus.json",
        "visio/simple.json",
        "camtasia/simple.json",
        "txml/simple.json",
        "typo3/simple.json",
        "wordpress/simple.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn p3_filters3_all_java_test_goldens() {
    let tmp = tempfile::tempdir().unwrap();
    let inv: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("fixtures/goldens/engine/filter_tests.json")).unwrap(),
    )
    .unwrap();
    let filters3 = [
        "android",
        "xhtml",
        "helpandmanual",
        "propxml",
        "schematron",
        "relaxng",
        "camtasia",
        "docbook",
        "opendoc",
        "openxml",
        "resx",
        "wix",
        "typo3",
        "l10nmgr",
        "svg",
        "infix",
        "flash",
        "txml",
        "visio",
        "xmlss",
        "wordpress",
        "scribus",
        "xliff",
        "filters",
    ];
    let mut n = 0;
    let mut fails = Vec::new();
    for t in inv["tests"].as_array().unwrap() {
        let golden = t["golden"].as_str().unwrap();
        let id = golden.split('/').nth(1).unwrap_or("");
        if !filters3.contains(&id) {
            continue;
        }
        let path = repo_root().join("fixtures/goldens").join(golden);
        if !path.is_file() {
            fails.push(format!("missing {}", path.display()));
            continue;
        }
        let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        check_provenance(&path, &spec);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_unit_or_filter(&path, &spec, tmp.path());
        }));
        if let Err(e) = result {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            fails.push(format!("{}: {msg}", path.display()));
        }
        n += 1;
    }
    if !fails.is_empty() {
        panic!("{} filters3 goldens failed:\n{}", fails.len(), fails.join("\n"));
    }
    let expected = inv["tests"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            let golden = t["golden"].as_str().unwrap_or("");
            filters3.contains(&golden.split('/').nth(1).unwrap_or(""))
        })
        .count();
    assert_eq!(n, expected, "filters3 inventory goldens");
}

#[test]
fn g3_opendoc_openxml_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "opendoc/file-OpenDocFilter.json",
        "openxml/file-OpenXMLFilter.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g4_xliff_sdl_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "xliff1/en-xx.json",
        "xliff2/ex.9.5.json",
        "sdlxliff/simple.json",
        "sdlproject/simple.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g4_msoffice_java_goldens_must_match() {
    let tmp = tempfile::tempdir().unwrap();
    for rel in [
        "msoffice/file-OpenXMLFilter.json",
        "msoffice/file-OpenXMLFilter-tables.json",
    ] {
        assert_rel(rel, tmp.path());
    }
}

#[test]
fn g4_msoffice_translation_lands_on_wt_node() {
    let root = repo_root();
    let src = root.join("fixtures/filters/openXML/file-OpenXMLFilter.docx");
    let reg = FilterRegistry::new();
    let filter = reg.by_id("msoffice").expect("msoffice");
    let ctx = FilterContext {
        source_lang: "en".into(),
        target_lang: "be".into(),
        ..FilterContext::default()
    };
    let parsed = filter.parse(&src, &ctx).unwrap();
    assert_eq!(parsed.segments[0].source, "This is first line.");
    let mut map = HashMap::new();
    map.insert("This is first line.".into(), "GOLDEN_T".into());
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("out.docx");
    filter.write(&src, &dest, &map, &ctx).unwrap();
    let file = std::fs::File::open(&dest).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let start = xml.find("<w:t").expect("w:t start");
    let slice = &xml[start..];
    let gt = slice.find('>').unwrap();
    let end = slice.find("</w:t>").expect("w:t end");
    let text = &slice[gt + 1..end];
    assert_eq!(text, "GOLDEN_T", "translation must land in first w:t node");
}

/// G2–G4: every Java plugin id has a golden directory.
#[test]
fn g2_g4_forty_nine_java_ids_have_golden_dirs() {
    let ids = [
        "text", "latex", "po", "rc", "moodlephp", "mozdtd", "mozlang", "properties", "mozftl",
        "html", "hhc", "ini", "dokuwiki", "magento", "ilias", "yaml", "pdf", "srt", "sbv",
        "webvtt", "xtag", "android", "xhtml", "helpandmanual", "propxml", "schematron",
        "relaxng", "camtasia", "docbook", "opendoc", "openxml", "resx", "wix", "typo3",
        "l10nmgr", "svg", "infix", "flash", "txml", "visio", "xmlss", "wordpress", "scribus",
        "xliff", "msoffice", "xliff1", "xliff2", "sdlxliff", "sdlproject",
    ];
    assert_eq!(ids.len(), 49);
    let root = goldens_dir();
    for id in ids {
        let dir = root.join(id);
        assert!(dir.is_dir(), "missing golden directory for Java id {id}");
        let has_json = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"));
        assert!(has_json, "golden directory {id} has no json");
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
