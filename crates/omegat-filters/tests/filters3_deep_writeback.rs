use omegat_filters::{FilterContext, FilterRegistry};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;

fn write_zip(path: &Path, parts: &[(&str, &str)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for (name, xml) in parts {
        zip.start_file(*name, FileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn read_part(path: &Path, name: &str) -> String {
    let file = std::fs::File::open(path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut xml = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut xml).unwrap();
    xml
}

fn element_names_and_text(xml: &str) -> (Vec<String>, Vec<String>) {
    let doc = roxmltree::Document::parse(xml).unwrap();
    let names = doc
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| node.tag_name().name().to_string())
        .collect();
    let text = doc
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .collect();
    (names, text)
}

#[test]
fn openxml_deep_writeback_targets_a_namespaced_part_occurrence() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("deep.docx");
    let output = temp.path().join("translated.docx");
    let document = r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>Same</w:t></w:r><w:r><w:t> deep</w:t></w:r></w:p></w:body></w:document>"#;
    let header = r#"<w:hdr xmlns:w="urn:w"><w:p><w:r><w:t>Same</w:t></w:r><w:r><w:t> deep</w:t></w:r></w:p></w:hdr>"#;
    write_zip(
        &source,
        &[
            ("word/document.xml", document),
            ("word/header1.xml", header),
        ],
    );

    let registry = FilterRegistry::new();
    let filter = registry.by_id("openxml").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                "word/document.xml#0",
                "<w0><w1>Same</w1></w0><w2><w3> deep</w3></w2>",
            ),
            (
                "word/header1.xml#0",
                "<w0><w1>Same</w1></w0><w2><w3> deep</w3></w2>",
            ),
        ]
    );

    let mut translations = HashMap::new();
    translations.insert(
        "word/header1.xml#0".into(),
        "<w0><w1>Header</w1></w0><w2><w3> only</w3></w2>".into(),
    );
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    let reparsed = filter.parse(&output, &context).unwrap();
    assert_eq!(
        reparsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                "word/document.xml#0",
                "<w0><w1>Same</w1></w0><w2><w3> deep</w3></w2>",
            ),
            (
                "word/header1.xml#0",
                "<w0><w1>Header</w1></w0><w2><w3> only</w3></w2>",
            ),
        ]
    );
    assert_eq!(
        element_names_and_text(&read_part(&output, "word/document.xml")),
        (
            vec!["document", "body", "p", "r", "t", "r", "t"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            vec!["Same", "deep"]
                .into_iter()
                .map(str::to_string)
                .collect()
        )
    );
    assert_eq!(
        element_names_and_text(&read_part(&output, "word/header1.xml")).1,
        vec!["Header", "only"]
    );
}

#[test]
fn opendoc_deep_writeback_distinguishes_content_meta_and_intact_styles() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("deep.odt");
    let output = temp.path().join("translated.odt");
    let content = r#"<office:document-content xmlns:office="urn:office" xmlns:text="urn:text"><office:body><office:text><text:p><text:span>Repeated</text:span></text:p></office:text></office:body></office:document-content>"#;
    let styles = r#"<office:document-styles xmlns:office="urn:office" xmlns:dc="urn:dc"><office:styles><dc:title>Repeated</dc:title></office:styles></office:document-styles>"#;
    let meta = r#"<office:document-meta xmlns:office="urn:office" xmlns:dc="urn:dc"><office:meta><dc:title>Repeated</dc:title></office:meta></office:document-meta>"#;
    write_zip(
        &source,
        &[
            ("content.xml", content),
            ("styles.xml", styles),
            ("meta.xml", meta),
        ],
    );

    let registry = FilterRegistry::new();
    let filter = registry.by_id("opendoc").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![("content.xml#0", "Repeated"), ("meta.xml#0", "Repeated")]
    );

    let mut translations = HashMap::new();
    translations.insert("meta.xml#0".into(), "Metadata title".into());
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    let reparsed = filter.parse(&output, &context).unwrap();
    assert_eq!(
        reparsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Repeated"),
            ("meta.xml#0", "Metadata title")
        ]
    );
    assert_eq!(
        element_names_and_text(&read_part(&output, "content.xml")).1,
        vec!["Repeated"]
    );
    assert_eq!(
        element_names_and_text(&read_part(&output, "styles.xml")).1,
        vec!["Repeated"]
    );
    assert_eq!(
        element_names_and_text(&read_part(&output, "meta.xml")).1,
        vec!["Metadata title"]
    );
}

#[test]
fn opendoc_attributes_and_out_of_turn_notes_write_by_unique_id() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("attributes-and-notes.odt");
    let output = temp.path().join("translated-attributes-and-notes.odt");
    let content = r#"<office:document-content xmlns:office="urn:office" xmlns:text="urn:text" xmlns:xlink="urn:xlink"><office:body><office:text><text:p><text:alphabetical-index-mark text:string-value="Index" text:key1="First" text:key2="Second"/></text:p><text:p><text:a xlink:href="Original link"/></text:p><text:p><text:note><text:note-body><text:p>Original note</text:p></text:note-body></text:note></text:p></office:text></office:body></office:document-content>"#;
    write_zip(&source, &[("content.xml", content)]);

    let registry = FilterRegistry::new();
    let filter = registry.by_id("opendoc").unwrap();
    let mut context = FilterContext::default();
    context
        .options
        .insert("translateLinks".into(), "true".into());
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Index"),
            ("content.xml#1", "First"),
            ("content.xml#2", "Second"),
            ("content.xml#3", "Original link"),
            ("content.xml#4", "Original note"),
        ]
    );

    let translations = HashMap::from([
        ("content.xml#4".into(), "Translated note".into()),
        ("content.xml#2".into(), "Translated second".into()),
        ("content.xml#0".into(), "Translated & \"index\"".into()),
        ("content.xml#3".into(), "Translated link".into()),
        ("content.xml#1".into(), "Translated first".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();

    let reparsed = filter.parse(&output, &context).unwrap();
    assert_eq!(
        reparsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Translated & \"index\""),
            ("content.xml#1", "Translated first"),
            ("content.xml#2", "Translated second"),
            ("content.xml#3", "Translated link"),
            ("content.xml#4", "Translated note"),
        ]
    );

    let rewritten = read_part(&output, "content.xml");
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let index = document
        .descendants()
        .find(|node| node.tag_name().name() == "alphabetical-index-mark")
        .unwrap();
    assert_eq!(
        index.attribute(("urn:text", "string-value")),
        Some("Translated & \"index\"")
    );
    assert_eq!(
        index.attribute(("urn:text", "key1")),
        Some("Translated first")
    );
    assert_eq!(
        index.attribute(("urn:text", "key2")),
        Some("Translated second")
    );
    let link = document
        .descendants()
        .find(|node| node.tag_name().name() == "a")
        .unwrap();
    assert_eq!(
        link.attribute(("urn:xlink", "href")),
        Some("Translated link")
    );
    let note_body = document
        .descendants()
        .find(|node| node.tag_name().name() == "note-body")
        .unwrap();
    let note_paragraph = note_body
        .children()
        .find(|node| node.tag_name().name() == "p")
        .unwrap();
    assert_eq!(note_paragraph.text(), Some("Translated note"));
}

#[test]
fn opendoc_nested_out_of_turn_and_attributes_share_one_stable_id_stream() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("nested-note-annotation.odt");
    let output = temp.path().join("translated-nested-note-annotation.odt");
    let content = r#"<office:document-content xmlns:office="urn:office" xmlns:text="urn:text" xmlns:xlink="urn:xlink"><office:body><office:text><text:p><text:note xlink:href="Note URL"><text:note-body><text:p>Outer <office:annotation xlink:href="Annotation URL"><text:p>Inner <text:a xlink:href="Inner URL">linked</text:a><text:alphabetical-index-mark text:string-value="Index" text:key1="First" text:key2="Second"/></text:p></office:annotation> tail</text:p></text:note-body></text:note></text:p></office:text></office:body></office:document-content>"#;
    write_zip(&source, &[("content.xml", content)]);

    let registry = FilterRegistry::new();
    let filter = registry.by_id("opendoc").unwrap();
    let mut context = FilterContext::default();
    context
        .options
        .insert("translateLinks".into(), "true".into());
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Note URL"),
            ("content.xml#1", "Annotation URL"),
            ("content.xml#2", "Inner URL"),
            ("content.xml#3", "Index"),
            ("content.xml#4", "First"),
            ("content.xml#5", "Second"),
            ("content.xml#6", "Inner <a0>linked</a0><i1/>"),
            ("content.xml#7", "Outer <o0/> tail"),
        ]
    );

    let translations = HashMap::from([
        ("content.xml#7".into(), "Extérieur <o0/> fin".into()),
        ("content.xml#3".into(), "Index traduit".into()),
        ("content.xml#0".into(), "Note & \"URL\"".into()),
        ("content.xml#6".into(), "Interne <a0>lié</a0><i1/>".into()),
        ("content.xml#5".into(), "Clé deux".into()),
        ("content.xml#2".into(), "URL interne".into()),
        ("content.xml#1".into(), "URL annotation".into()),
        ("content.xml#4".into(), "Clé un".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    let reparsed = filter.parse(&output, &context).unwrap();
    assert_eq!(
        reparsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Note & \"URL\""),
            ("content.xml#1", "URL annotation"),
            ("content.xml#2", "URL interne"),
            ("content.xml#3", "Index traduit"),
            ("content.xml#4", "Clé un"),
            ("content.xml#5", "Clé deux"),
            ("content.xml#6", "Interne <a0>lié</a0><i1/>"),
            ("content.xml#7", "Extérieur <o0/> fin"),
        ]
    );

    let rewritten = read_part(&output, "content.xml");
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let note = document
        .descendants()
        .find(|node| node.tag_name().name() == "note")
        .unwrap();
    let annotation = document
        .descendants()
        .find(|node| node.tag_name().name() == "annotation")
        .unwrap();
    let link = document
        .descendants()
        .find(|node| node.tag_name().name() == "a")
        .unwrap();
    let index = document
        .descendants()
        .find(|node| node.tag_name().name() == "alphabetical-index-mark")
        .unwrap();
    assert_eq!(
        note.attribute(("urn:xlink", "href")),
        Some("Note & \"URL\"")
    );
    assert_eq!(
        annotation.attribute(("urn:xlink", "href")),
        Some("URL annotation")
    );
    assert_eq!(link.attribute(("urn:xlink", "href")), Some("URL interne"));
    assert_eq!(
        (
            index.attribute(("urn:text", "string-value")),
            index.attribute(("urn:text", "key1")),
            index.attribute(("urn:text", "key2")),
        ),
        (Some("Index traduit"), Some("Clé un"), Some("Clé deux"))
    );
    assert_eq!(
        element_names_and_text(&rewritten).1,
        vec!["Extérieur", "Interne", "lié", "fin"]
    );
}
