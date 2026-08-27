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
fn openxml_hidden_text_external_link_and_intact_fallback_write_independently() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("dialect-options.docx");
    let output = temp.path().join("translated-dialect-options.docx");
    let document = r#"<w:document xmlns:w="urn:w" xmlns:mc="urn:mc"><w:body><w:p><w:instrText>Hidden field</w:instrText></w:p><w:p><mc:Fallback><w:r><w:t>Fallback text</w:t></w:r></mc:Fallback></w:p></w:body></w:document>"#;
    let relationships = r#"<Relationships><Relationship Id="external" TargetMode="External" Target="https://example.test/original?a=1&amp;b=2"/><Relationship Id="internal" Target="media/image.png"/></Relationships>"#;
    write_zip(
        &source,
        &[
            ("word/document.xml", document),
            ("word/_rels/document.xml.rels", relationships),
        ],
    );

    let registry = FilterRegistry::new();
    let filter = registry.by_id("openxml").unwrap();
    let mut context = FilterContext::default();
    context
        .options
        .insert("translateHiddenText".into(), "true".into());
    context
        .options
        .insert("translateSlideLinks".into(), "true".into());
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("word/document.xml#0", "Hidden field"),
            (
                "word/_rels/document.xml.rels#0",
                "https://example.test/original?a=1&b=2",
            ),
        ]
    );

    let translations = HashMap::from([
        (
            "word/_rels/document.xml.rels#0".into(),
            "https://example.test/traduit?a=3&b=4".into(),
        ),
        ("word/document.xml#0".into(), "Champ masqué".into()),
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
            ("word/document.xml#0", "Champ masqué"),
            (
                "word/_rels/document.xml.rels#0",
                "https://example.test/traduit?a=3&b=4",
            ),
        ]
    );

    let rewritten_document = read_part(&output, "word/document.xml");
    let document = roxmltree::Document::parse(&rewritten_document).unwrap();
    assert_eq!(
        document
            .descendants()
            .find(|node| node.tag_name().name() == "instrText")
            .and_then(|node| node.text()),
        Some("Champ masqué")
    );
    assert_eq!(
        document
            .descendants()
            .find(|node| node.tag_name().name() == "t")
            .and_then(|node| node.text()),
        Some("Fallback text")
    );
    let rewritten_relationships = read_part(&output, "word/_rels/document.xml.rels");
    let relationships = roxmltree::Document::parse(&rewritten_relationships).unwrap();
    let targets: Vec<_> = relationships
        .descendants()
        .filter(|node| node.tag_name().name() == "Relationship")
        .map(|node| {
            (
                node.attribute("Id").unwrap(),
                node.attribute("Target").unwrap(),
            )
        })
        .collect();
    assert_eq!(
        targets,
        vec![
            ("external", "https://example.test/traduit?a=3&b=4",),
            ("internal", "media/image.png"),
        ]
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
            ("content.xml#6", "Inner <a0>linked</a0>"),
            ("content.xml#7", "Outer <o0/> tail"),
        ]
    );

    let translations = HashMap::from([
        ("content.xml#7".into(), "Extérieur <o0/> fin".into()),
        ("content.xml#3".into(), "Index traduit".into()),
        ("content.xml#0".into(), "Note & \"URL\"".into()),
        ("content.xml#6".into(), "Interne <a0>lié</a0>".into()),
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
            ("content.xml#6", "Interne <a0>lié</a0>"),
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

#[test]
fn xliff_nested_sub_writeback_preserves_depth_with_out_of_order_translations() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("nested.xlf");
    let output = temp.path().join("translated.xlf");
    std::fs::write(
        &source,
        r#"<xliff version="1.2"><file><body><trans-unit id="unit"><source>Source</source><target state="new">Before <sub>Outer <bpt id="1">&lt;b&gt;</bpt>bold<ept id="1">&lt;/b&gt;</ept> tail</sub> after</target></trans-unit></body></file></xliff>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("xliff").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("unit#0", "Outer <b0>bold</b0> tail"),
            ("unit#1", "Before <s0/> after"),
        ]
    );

    let translations = HashMap::from([
        ("unit#1".into(), "Avant <s0/> après".into()),
        ("unit#0".into(), "Extérieur <b0>gras</b0> fin".into()),
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
            ("unit#0", "Extérieur <b0>gras</b0> fin"),
            ("unit#1", "Avant <s0/> après"),
        ]
    );
    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let target = document
        .descendants()
        .find(|node| node.tag_name().name() == "target")
        .unwrap();
    assert_eq!(target.attribute("state"), Some("translated"));
    let subs: Vec<_> = target
        .descendants()
        .filter(|node| node.tag_name().name() == "sub")
        .collect();
    assert_eq!(subs.len(), 1);
    assert_eq!(
        target
            .descendants()
            .filter(|node| node.tag_name().name() == "bpt")
            .count(),
        1
    );
    assert_eq!(
        target
            .descendants()
            .filter(|node| node.tag_name().name() == "ept")
            .count(),
        1
    );
    assert_eq!(
        element_names_and_text(&rewritten).1,
        vec![
            "Source",
            "Avant",
            "Extérieur",
            "<b>",
            "gras",
            "</b>",
            "fin",
            "après",
        ]
    );
}

#[test]
fn docbook_nested_indexterm_attributes_and_text_share_stable_ids() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("nested.dbk");
    let output = temp.path().join("translated.dbk");
    std::fs::write(
        &source,
        r#"<book><para url="Outer URL">Before <indexterm url="Index URL"><primary>Outer index <indexterm xml:lang="en"><secondary>Inner index</secondary></indexterm> tail</primary></indexterm> after</para></book>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("docbook").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("0", "Outer URL"),
            ("1", "Index URL"),
            ("2", "en"),
            ("3", "Inner index"),
            ("4", "Outer index <i0/> tail"),
            ("5", "Before <i0/> after"),
        ]
    );

    let translations = HashMap::from([
        ("5".into(), "Avant <i0/> après".into()),
        ("2".into(), "fr".into()),
        ("4".into(), "Index extérieur <i0/> fin".into()),
        ("0".into(), "URL extérieure".into()),
        ("3".into(), "Index intérieur".into()),
        ("1".into(), "URL index".into()),
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
            ("0", "URL extérieure"),
            ("1", "URL index"),
            ("2", "fr"),
            ("3", "Index intérieur"),
            ("4", "Index extérieur <i0/> fin"),
            ("5", "Avant <i0/> après"),
        ]
    );
    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let para = document
        .descendants()
        .find(|node| node.tag_name().name() == "para")
        .unwrap();
    let terms: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "indexterm")
        .collect();
    assert_eq!(terms.len(), 2);
    assert_eq!(para.attribute("url"), Some("URL extérieure"));
    assert_eq!(terms[0].attribute("url"), Some("URL index"));
    assert_eq!(
        terms[1].attribute(("http://www.w3.org/XML/1998/namespace", "lang")),
        Some("fr")
    );
    assert_eq!(
        element_names_and_text(&rewritten).1,
        vec![
            "Avant",
            "Index extérieur",
            "Index intérieur",
            "fin",
            "après",
        ]
    );
}

#[test]
fn xhtml_option_matrix_skips_regex_meta_and_intact_content_during_writeback() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("options.xhtml");
    let output = temp.path().join("translated.xhtml");
    std::fs::write(
        &source,
        r#"<html><body><p>SKIP ME</p><meta name="robots" content="NOINDEX"/><input type="text" value="Plain value"/><input type="button" value="Click"/><p class="locked">Locked <span title="Hidden title">nested</span></p><p><a href="https://example.test/original" hreflang="en">Visible link</a><br/>After break</p></body></html>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("xhtml").unwrap();
    let mut context = FilterContext::default();
    context.options.extend([
        ("ignoreDoctype".into(), "true".into()),
        ("skipRegExp".into(), "skip me".into()),
        ("skipMeta".into(), "name=robots".into()),
        ("ignoreTags".into(), "class=locked".into()),
        ("translateValue".into(), "false".into()),
        ("translateButtonValue".into(), "true".into()),
        ("paragraphOnBr".into(), "true".into()),
    ]);
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("0", "Click"),
            ("1", "https://example.test/original"),
            ("2", "en"),
            ("3", "Visible link"),
            ("4", "After break"),
        ]
    );

    let translations = HashMap::from([
        ("4".into(), "Après le saut".into()),
        ("2".into(), "fr".into()),
        ("0".into(), "Appuyer".into()),
        ("3".into(), "Lien visible".into()),
        ("1".into(), "https://example.test/traduit".into()),
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
            ("0", "Appuyer"),
            ("1", "https://example.test/traduit"),
            ("2", "fr"),
            ("3", "Lien visible"),
            ("4", "Après le saut"),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let inputs: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "input")
        .collect();
    assert_eq!(inputs[0].attribute("value"), Some("Plain value"));
    assert_eq!(inputs[1].attribute("value"), Some("Appuyer"));
    let meta = document
        .descendants()
        .find(|node| node.tag_name().name() == "meta")
        .unwrap();
    assert_eq!(meta.attribute("content"), Some("NOINDEX"));
    let locked = document
        .descendants()
        .find(|node| node.attribute("class") == Some("locked"))
        .unwrap();
    assert_eq!(locked.text(), Some("Locked "));
    let locked_span = locked
        .descendants()
        .find(|node| node.tag_name().name() == "span")
        .unwrap();
    assert_eq!(locked_span.attribute("title"), Some("Hidden title"));
    assert_eq!(locked_span.text(), Some("nested"));
    assert!(rewritten.contains(">SKIP ME<"));
}

#[test]
fn opendoc_disabled_regions_and_enabled_attributes_write_independently() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("option-matrix.ods");
    let output = temp.path().join("translated-option-matrix.ods");
    let content = r#"<office:document-content xmlns:office="urn:office" xmlns:text="urn:text" xmlns:table="urn:table" xmlns:xlink="urn:xlink" xmlns:presentation="urn:presentation"><office:body><office:text><table:table table:name="Original sheet"><text:p><text:bookmark-start text:name="Original bookmark"/></text:p><text:p><text:bookmark-ref text:ref-name="Original bookmark">Reference text</text:bookmark-ref></text:p><text:p><text:a xlink:href="https://example.test/original">Link text</text:a></text:p><text:p><text:note><text:note-body><text:p>Note text</text:p></text:note-body></text:note></text:p><text:p><office:annotation><text:p>Comment text</text:p></office:annotation></text:p><text:p><presentation:notes><text:p>Slide note</text:p></presentation:notes></text:p></table:table></office:text></office:body></office:document-content>"#;
    write_zip(&source, &[("content.xml", content)]);

    let registry = FilterRegistry::new();
    let filter = registry.by_id("opendoc").unwrap();
    let mut context = FilterContext::default();
    context.options.extend([
        ("translateBookmarks".into(), "true".into()),
        ("translateBookmarkRefs".into(), "false".into()),
        ("translateNotes".into(), "false".into()),
        ("translateComments".into(), "false".into()),
        ("translatePresNotes".into(), "false".into()),
        ("translateLinks".into(), "true".into()),
        ("translateSheetNames".into(), "true".into()),
    ]);
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("content.xml#0", "Original sheet"),
            ("content.xml#1", "Original bookmark"),
            ("content.xml#2", "https://example.test/original"),
            ("content.xml#3", "Link text"),
        ]
    );

    let translations = HashMap::from([
        ("content.xml#3".into(), "Texte du lien".into()),
        ("content.xml#1".into(), "Signet traduit".into()),
        ("content.xml#0".into(), "Feuille traduite".into()),
        (
            "content.xml#2".into(),
            "https://example.test/traduit".into(),
        ),
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
            ("content.xml#0", "Feuille traduite"),
            ("content.xml#1", "Signet traduit"),
            ("content.xml#2", "https://example.test/traduit"),
            ("content.xml#3", "Texte du lien"),
        ]
    );

    let rewritten = read_part(&output, "content.xml");
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let table = document
        .descendants()
        .find(|node| node.tag_name().name() == "table")
        .unwrap();
    assert_eq!(
        table.attribute(("urn:table", "name")),
        Some("Feuille traduite")
    );
    let bookmark_ref = document
        .descendants()
        .find(|node| node.tag_name().name() == "bookmark-ref")
        .unwrap();
    assert_eq!(
        bookmark_ref.attribute(("urn:text", "ref-name")),
        Some("Original bookmark")
    );
    assert_eq!(bookmark_ref.text(), Some("Reference text"));
    for untouched in ["Note text", "Comment text", "Slide note"] {
        assert!(
            document
                .descendants()
                .filter_map(|node| node.text())
                .any(|text| text == untouched),
            "{untouched} must remain intact"
        );
    }
}

#[test]
fn xliff_double_nested_sub_and_content_tags_write_back_in_depth_first_id_order() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("double-nested.xlf");
    let output = temp.path().join("translated-double-nested.xlf");
    std::fs::write(
        &source,
        r#"<xliff version="1.2"><file><body><trans-unit id="nested"><source>Source</source><target state="new">Before <sub>Outer <bpt id="1">&lt;b&gt;</bpt>bold<ept id="1">&lt;/b&gt;</ept> <sub>Nested <ph id="2">&lt;br/&gt;</ph> text</sub> tail</sub> after</target></trans-unit></body></file></xliff>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("xliff").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("nested#0", "Nested <b0/> text"),
            ("nested#1", "Outer <b0>bold</b0> <s2/> tail"),
            ("nested#2", "Before <s0/> after"),
        ]
    );
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| {
                segment
                    .protected_parts
                    .iter()
                    .map(|part| (part.text.as_str(), part.details.as_str()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![("<b0/>", r#"<ph id="2">&lt;br/&gt;</ph>"#)],
            vec![
                ("<b0>", r#"<bpt id="1">&lt;b&gt;</bpt>"#),
                ("</b0>", r#"<ept id="1">&lt;/b&gt;</ept>"#),
            ],
            vec![],
        ]
    );

    let translations = HashMap::from([
        ("nested#2".into(), "Avant <s0/> après".into()),
        ("nested#0".into(), "Imbriqué <b0/> texte".into()),
        (
            "nested#1".into(),
            "Extérieur <b0>gras</b0> <s2/> fin".into(),
        ),
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
            ("nested#0", "Imbriqué <b0/> texte"),
            ("nested#1", "Extérieur <b0>gras</b0> <s2/> fin"),
            ("nested#2", "Avant <s0/> après"),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let target = document
        .descendants()
        .find(|node| node.tag_name().name() == "target")
        .unwrap();
    let subs: Vec<_> = target
        .descendants()
        .filter(|node| node.tag_name().name() == "sub")
        .collect();
    assert_eq!(subs.len(), 2);
    assert_eq!(
        subs[1]
            .parent()
            .filter(|node| node.tag_name().name() == "sub"),
        Some(subs[0])
    );
    assert_eq!(
        (
            target
                .descendants()
                .filter(|node| node.tag_name().name() == "bpt")
                .count(),
            target
                .descendants()
                .filter(|node| node.tag_name().name() == "ept")
                .count(),
            target
                .descendants()
                .filter(|node| node.tag_name().name() == "ph")
                .count(),
        ),
        (1, 1, 1)
    );
    assert_eq!(
        element_names_and_text(&rewritten).1,
        vec![
            "Source",
            "Avant",
            "Extérieur",
            "<b>",
            "gras",
            "</b>",
            "Imbriqué",
            "<br/>",
            "texte",
            "fin",
            "après",
        ]
    );
}

#[test]
fn xhtml_nested_intact_and_inline_attributes_keep_independent_writeback_ids() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("nested-options.xhtml");
    let output = temp.path().join("translated-nested-options.xhtml");
    std::fs::write(
        &source,
        r#"<html><body><div><p title="Outer title">Visible <span title="Inline title">inner</span></p><section class="locked" title="Locked title"><p>Locked <span title="Locked nested title">nested</span></p></section><p>Before break<br/>After break</p></div></body></html>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("xhtml").unwrap();
    let mut context = FilterContext::default();
    context.options.extend([
        ("ignoreDoctype".into(), "true".into()),
        ("ignoreTags".into(), "class=locked".into()),
        ("paragraphOnBr".into(), "true".into()),
    ]);
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("0", "Outer title"),
            ("1", "Inline title"),
            ("2", "Visible <s0>inner</s0>"),
            ("3", "Before break"),
            ("4", "After break"),
        ]
    );

    let translations = HashMap::from([
        ("4".into(), "Après la rupture".into()),
        ("2".into(), "Visible <s0>intérieur</s0>".into()),
        ("0".into(), "Titre extérieur".into()),
        ("3".into(), "Avant la rupture".into()),
        ("1".into(), "Titre en ligne".into()),
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
            ("0", "Titre extérieur"),
            ("1", "Titre en ligne"),
            ("2", "Visible <s0>intérieur</s0>"),
            ("3", "Avant la rupture"),
            ("4", "Après la rupture"),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let paragraphs: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "p")
        .collect();
    assert_eq!(paragraphs[0].attribute("title"), Some("Titre extérieur"));
    let translated_span = paragraphs[0]
        .descendants()
        .find(|node| node.tag_name().name() == "span")
        .unwrap();
    assert_eq!(translated_span.attribute("title"), Some("Titre en ligne"));
    assert_eq!(translated_span.text(), Some("intérieur"));
    let locked = document
        .descendants()
        .find(|node| node.attribute("class") == Some("locked"))
        .unwrap();
    assert_eq!(locked.attribute("title"), Some("Locked title"));
    let locked_span = locked
        .descendants()
        .find(|node| node.tag_name().name() == "span")
        .unwrap();
    assert_eq!(
        (locked_span.attribute("title"), locked_span.text(),),
        (Some("Locked nested title"), Some("nested"))
    );
    assert_eq!(paragraphs[2].text(), Some("Avant la rupture"));
    assert_eq!(
        paragraphs[2]
            .children()
            .filter(|node| node.tag_name().name() == "br")
            .count(),
        1
    );
    assert_eq!(
        paragraphs[2]
            .children()
            .filter_map(|node| node.text())
            .collect::<Vec<_>>(),
        vec!["Avant la rupture", "Après la rupture"]
    );
}

#[test]
fn xliff_duplicate_unit_ids_keep_nested_callback_streams_independent() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("duplicate-units.xlf");
    let output = temp.path().join("translated-duplicate-units.xlf");
    std::fs::write(
        &source,
        r#"<xliff version="1.2"><file><body><trans-unit id="duplicate"><source>First source</source><target state="new">First outer <sub>First inner</sub> tail</target></trans-unit><trans-unit id="duplicate"><source>Second source</source><target state="new">Second outer <sub>Second inner</sub> tail</target></trans-unit></body></file></xliff>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("xliff").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("duplicate#0", "First inner"),
            ("duplicate#1", "First outer <s0/> tail"),
            ("duplicate_1#0", "Second inner"),
            ("duplicate_1#1", "Second outer <s0/> tail"),
        ]
    );

    let translations = HashMap::from([
        (
            "duplicate_1#1".into(),
            "Deuxième extérieur <s0/> fin".into(),
        ),
        ("duplicate#0".into(), "Premier intérieur".into()),
        ("duplicate#1".into(), "Premier extérieur <s0/> fin".into()),
        ("duplicate_1#0".into(), "Deuxième intérieur".into()),
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
            ("duplicate#0", "Premier intérieur"),
            ("duplicate#1", "Premier extérieur <s0/> fin"),
            ("duplicate_1#0", "Deuxième intérieur"),
            ("duplicate_1#1", "Deuxième extérieur <s0/> fin"),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let targets: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "target")
        .collect();
    assert_eq!(targets.len(), 2);
    let first_sub = targets[0]
        .descendants()
        .find(|node| node.tag_name().name() == "sub")
        .unwrap();
    let second_sub = targets[1]
        .descendants()
        .find(|node| node.tag_name().name() == "sub")
        .unwrap();
    assert_eq!(first_sub.text(), Some("Premier intérieur"));
    assert_eq!(second_sub.text(), Some("Deuxième intérieur"));
    assert_eq!(
        element_names_and_text(&rewritten).1,
        vec![
            "First source",
            "Premier extérieur",
            "Premier intérieur",
            "fin",
            "Second source",
            "Deuxième extérieur",
            "Deuxième intérieur",
            "fin",
        ]
    );
}

#[test]
fn openxml_nested_callbacks_write_to_reversed_zip_parts_by_qualified_id() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("reversed-parts.docx");
    let output = temp.path().join("translated-reversed-parts.docx");
    let header = r#"<w:hdr xmlns:w="urn:w"><w:p><w:instrText>Header hidden</w:instrText><w:r><w:t>Header visible</w:t></w:r></w:p></w:hdr>"#;
    let document = r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:instrText>Document hidden</w:instrText><w:r><w:t>Document visible</w:t></w:r></w:p></w:body></w:document>"#;
    write_zip(
        &source,
        &[
            ("word/header10.xml", header),
            ("word/header2.xml", header),
            ("word/document.xml", document),
        ],
    );

    let registry = FilterRegistry::new();
    let filter = registry.by_id("openxml").unwrap();
    let mut context = FilterContext::default();
    context
        .options
        .insert("translateHiddenText".into(), "true".into());
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("word/document.xml#0", "Document hidden"),
            ("word/document.xml#1", "Document visible"),
            ("word/header2.xml#0", "Header hidden"),
            ("word/header2.xml#1", "Header visible"),
            ("word/header10.xml#0", "Header hidden"),
            ("word/header10.xml#1", "Header visible"),
        ]
    );

    let translations = HashMap::from([
        ("word/header10.xml#1".into(), "En-tête dix visible".into()),
        ("word/document.xml#0".into(), "Document masqué".into()),
        ("word/header2.xml#0".into(), "En-tête deux masqué".into()),
        (
            "word/document.xml#1".into(),
            "Document visible traduit".into(),
        ),
        ("word/header10.xml#0".into(), "En-tête dix masqué".into()),
        ("word/header2.xml#1".into(), "En-tête deux visible".into()),
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
            ("word/document.xml#0", "Document masqué"),
            ("word/document.xml#1", "Document visible traduit"),
            ("word/header2.xml#0", "En-tête deux masqué"),
            ("word/header2.xml#1", "En-tête deux visible"),
            ("word/header10.xml#0", "En-tête dix masqué"),
            ("word/header10.xml#1", "En-tête dix visible"),
        ]
    );

    for (part, hidden, visible) in [
        (
            "word/document.xml",
            "Document masqué",
            "Document visible traduit",
        ),
        (
            "word/header2.xml",
            "En-tête deux masqué",
            "En-tête deux visible",
        ),
        (
            "word/header10.xml",
            "En-tête dix masqué",
            "En-tête dix visible",
        ),
    ] {
        let rewritten = read_part(&output, part);
        let document = roxmltree::Document::parse(&rewritten).unwrap();
        assert_eq!(
            document
                .descendants()
                .find(|node| node.tag_name().name() == "instrText")
                .and_then(|node| node.text()),
            Some(hidden)
        );
        assert_eq!(
            document
                .descendants()
                .find(|node| node.tag_name().name() == "t")
                .and_then(|node| node.text()),
            Some(visible)
        );
    }
}

#[test]
fn android_comments_intact_resources_and_empty_plural_write_back_by_named_id() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("deep-android.xml");
    let output = temp.path().join("translated-deep-android.xml");
    std::fs::write(
        &source,
        r#"<resources xmlns:xliff="urn:oasis:names:tc:xliff:document:1.2"><!--Translator note--><string name="title">Don\'t <b>stop</b></string><!--DO NOT TRANSLATE generated value--><string name="locked">Locked</string><string name="flag" translatable="false">Invisible</string><plurals name="count"><item quantity="one">One item</item><item quantity="other"><xliff:g id="count">%d</xliff:g> items</item></plurals></resources>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.by_id("android").unwrap();
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        parsed
            .segments
            .iter()
            .map(|segment| {
                (
                    segment.id.as_str(),
                    segment.source.as_str(),
                    segment.note.as_deref(),
                    segment
                        .protected_parts
                        .iter()
                        .map(|part| (part.text.as_str(), part.details.as_str()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "title",
                "Don't <b0>stop</b0>",
                Some("Translator note"),
                vec![("<b0>", "<b>"), ("</b0>", "</b>")],
            ),
            ("count/one", "One item", None, vec![]),
            (
                "count/other",
                "<x0>%d</x0> items",
                None,
                vec![
                    ("<x0>", r#"<xliff:g id="count">"#),
                    ("</x0>", "</xliff:g>"),
                ],
            ),
        ]
    );

    let translations = HashMap::from([
        (
            "count/other".into(),
            "<x0>%d</x0> éléments".into(),
        ),
        ("count/one".into(), String::new()),
        ("title".into(), "N'arrêtez <b0>jamais</b0>".into()),
        ("locked".into(), "Must not be written".into()),
        ("flag".into(), "Must stay intact".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let strings: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "string")
        .map(|node| {
            (
                node.attribute("name"),
                node.attribute("translatable"),
                node.text(),
                node.descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>(),
            )
        })
        .collect();
    assert_eq!(
        strings,
        vec![
            (
                Some("title"),
                None,
                Some(r#"N\'arrêtez "#),
                r#"N\'arrêtez jamais"#.to_string(),
            ),
            (Some("locked"), None, Some("Locked"), "Locked".to_string()),
            (
                Some("flag"),
                Some("false"),
                Some("Invisible"),
                "Invisible".to_string(),
            ),
        ]
    );
    let plural_items: Vec<_> = document
        .descendants()
        .filter(|node| node.tag_name().name() == "item")
        .map(|node| {
            (
                node.attribute("quantity"),
                node.text(),
                node.descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>(),
            )
        })
        .collect();
    assert_eq!(
        plural_items,
        vec![
            (Some("one"), None, String::new()),
            (
                Some("other"),
                None,
                "%d éléments".to_string(),
            ),
        ]
    );

    let reparsed = filter.parse(&output, &context).unwrap();
    assert_eq!(
        reparsed
            .segments
            .iter()
            .map(|segment| (segment.id.as_str(), segment.source.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("title", "N'arrêtez <b0>jamais</b0>"),
            ("count/other", "<x0>%d</x0> éléments"),
        ]
    );
}
