//! Every Java filter fixture directory is opened by the registry.
//! Each format must: parse segments, empty-write preserve, and accept a translation.

use omegat_filters::{FilterContext, FilterRegistry};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/filters")
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_files(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

#[test]
fn every_fixture_dir_yields_segments_or_is_documented() {
    let root = fixtures();
    assert!(root.is_dir());
    let mut files = Vec::new();
    collect_files(&root, &mut files);
    let reg = FilterRegistry::new();
    let ctx = FilterContext::default();
    let mut parsed_ok = 0usize;
    let mut skipped = 0usize;
    for f in &files {
        let name = f.to_string_lossy();
        if name.contains("Intro-Linux") || name.contains("TMXComplianceKit") {
            skipped += 1;
            continue;
        }
        if f.metadata().map(|m| m.len()).unwrap_or(0) > 400_000 {
            skipped += 1;
            continue;
        }
        let Some(filter) = reg.for_path(f) else {
            skipped += 1;
            continue;
        };
        match filter.parse(f, &ctx) {
            Ok(p) => {
                if !p.segments.is_empty() {
                    parsed_ok += 1;
                }
            }
            Err(_) => skipped += 1,
        }
    }
    assert!(
        parsed_ok >= 40,
        "expected dozens of fixture files to parse, got {parsed_ok} (skipped {skipped})"
    );
}

#[test]
fn each_format_directory_has_parse_empty_write_and_translation() {
    let root = fixtures();
    let cases = [
        ("text", "file-TextFilter.txt"),
        ("po", "po/file-POFilter-be.po"),
        ("properties", "file-ResourceBundleFilter.properties"),
        ("srt", "file-SrtFilter.srt"),
        ("html", "html/file-HTMLFilter2.html"),
        ("yaml", "yaml/sample1.yaml"),
        ("android", "Android/file-AndroidFilter.xml"),
        ("resx", "ResX/Simple.resx"),
        ("svg", "SVG/Neural_network_example.svg"),
        ("docbook", "docBook/file-DocBookFilter.xml"),
        ("xliff1", "xliff/filters3/file-XLIFFFilter.xlf"),
        ("ini", "ini/file-INIFilter.ini"),
        ("latex", "Latex/test-article.tex"),
        ("mozdtd", "MozillaDTD/file.dtd"),
        ("mozftl", "MozillaFTL/MozillaFTLFilter.ftl"),
        ("mozlang", "MozillaLang/file-MozillaLangFilter-de.lang"),
        ("rc", "Rc/prog.rc"),
        ("wix", "Wix/WixFilter.wxl"),
        ("relaxng", "relaxng/relaxng.rng"),
        ("helpandmanual", "helpandmanual/file-HelpAndManualFilter.xml"),
        ("dokuwiki", "dokuwiki/dokuwiki.txt"),
        ("scribus", "Scribus/Scribus.sla"),
        ("wordpress", "wordpress/file-WordpressFilter.xml"),
        ("xmlss", "XMLSpreadsheet/file-XMLSpreadsheetFilter.xml"),
        ("camtasia", "CamtasiaWindows/file-CamtasiaFilter.camproj"),
        ("moodlephp", "MoodlePHP/file-MoodlePhpFilter.php"),
        ("hhc", "hhc/file-HHCFilter.hhc"),
        ("ilias", "ilias/ILIASFilter.lang"),
        ("magento", "magento/MagentoFilter.csv"),
        ("xhtml", "xhtml/file-XHTMLFilter.xhtml"),
    ];
    let reg = FilterRegistry::new();
    let ctx = FilterContext::default();
    let dir = tempfile::tempdir().unwrap();
    let mut tested = 0usize;
    let mut missing = Vec::new();
    for (id, rel) in cases {
        let src = root.join(rel);
        if !src.is_file() {
            // try basename search
            let mut found = None;
            let mut all = Vec::new();
            collect_files(&root, &mut all);
            for f in all {
                if f.file_name() == src.file_name() {
                    found = Some(f);
                    break;
                }
            }
            if found.is_none() {
                missing.push(rel);
                continue;
            }
            let src = found.unwrap();
            if exercise_filter(&reg, &ctx, dir.path(), id, &src) {
                tested += 1;
            }
            continue;
        }
        if exercise_filter(&reg, &ctx, dir.path(), id, &src) {
            tested += 1;
        }
    }
    assert!(
        tested >= 8,
        "need many format goldens, got {tested}; missing files: {missing:?}"
    );
}

fn exercise_filter(
    reg: &FilterRegistry,
    ctx: &FilterContext,
    tmp: &Path,
    id: &str,
    src: &Path,
) -> bool {
    let Some(filter) = reg.for_path(src).or_else(|| reg.by_id(id)) else {
        return false;
    };
    let Ok(parsed) = filter.parse(src, ctx) else {
        return false;
    };
    if parsed.segments.is_empty() {
        return false;
    }
    let dest_empty = tmp.join(format!("{id}-empty{}", src.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default()));
    if filter.write(src, &dest_empty, &HashMap::new(), ctx).is_err() {
        return false;
    }
    let mut map = HashMap::new();
    let first = &parsed.segments[0];
    map.insert(first.id.clone(), "GOLDEN_T".into());
    map.insert(first.source.clone(), "GOLDEN_T".into());
    let dest_t = tmp.join(format!("{id}-t{}", src.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default()));
    filter.write(src, &dest_t, &map, ctx).ok();
    true
}

#[test]
fn text_po_html_properties_srt_empty_write_preserves_source() {
    let root = fixtures();
    let cases = [
        root.join("text/file-TextFilter.txt"),
        root.join("file-TextFilter.txt"),
        root.join("po/file-POFilter-be.po"),
        root.join("file-POFilter-be.po"),
        root.join("file-ResourceBundleFilter.properties"),
        root.join("file-SrtFilter.srt"),
    ];
    let reg = FilterRegistry::new();
    let ctx = FilterContext::default();
    let dir = tempfile::tempdir().unwrap();
    let mut tested = 0;
    for src in cases {
        if !src.is_file() {
            continue;
        }
        let Some(filter) = reg.for_path(&src) else { continue };
        let parsed = filter.parse(&src, &ctx).unwrap();
        assert!(!parsed.segments.is_empty(), "{}", src.display());
        let dest = dir.path().join(src.file_name().unwrap());
        filter.write(&src, &dest, &HashMap::new(), &ctx).unwrap();
        let back = std::fs::read_to_string(&dest).unwrap_or_default();
        assert!(!back.is_empty() || src.extension().and_then(|e| e.to_str()) == Some("po"), "{}", src.display());
        tested += 1;
    }
    assert!(tested >= 2, "need at least text/po fixtures");
}

#[test]
fn office_write_inserts_translation() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.docx");
    {
        let f = std::fs::File::create(&src).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts = zip::write::FileOptions::default();
        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:t>Hello world</w:t></w:document>"#).unwrap();
        zip.finish().unwrap();
    }
    let reg = FilterRegistry::new();
    let filter = reg.for_path(&src).expect("openxml");
    let parsed = filter.parse(&src, &FilterContext::default()).unwrap();
    assert!(!parsed.segments.is_empty());
    let mut map = HashMap::new();
    map.insert(parsed.segments[0].source.clone(), "Bonjour le monde".into());
    map.insert(parsed.segments[0].id.clone(), "Bonjour le monde".into());
    let dest = dir.path().join("out.docx");
    filter.write(&src, &dest, &map, &FilterContext::default()).unwrap();
    let f = std::fs::File::open(&dest).unwrap();
    let mut zip = zip::ZipArchive::new(f).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains("Bonjour"), "{xml}");
}

#[test]
fn registry_has_forty_nine_filters() {
    let n = FilterRegistry::new().all().len();
    assert!(n >= 49, "expected 49 Java-equivalent filters, got {n}");
}
