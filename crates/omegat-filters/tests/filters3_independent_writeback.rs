use omegat_filters::{FilterContext, FilterRegistry, ParsedFile};
use std::collections::HashMap;

fn segment_pairs(parsed: &ParsedFile) -> Vec<(String, String)> {
    parsed
        .segments
        .iter()
        .map(|segment| (segment.id.clone(), segment.source.clone()))
        .collect()
}

fn descendant_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

#[test]
fn properties_xml_duplicate_entries_and_intact_entry_write_by_occurrence_id() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("messages.xml");
    let output = temp.path().join("messages-fr.xml");
    std::fs::write(
        &source,
        r#"<properties><entry key="first">Same <b>one</b></entry><entry key="second">Same <i>two</i></entry><entry key="locked" translate="false">Locked</entry></properties>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "propxml");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "Same <b0>one</b0>".into()),
            ("1".into(), "Same <i0>two</i0>".into()),
        ]
    );

    let translations = HashMap::from([
        ("Same <b0>one</b0>".into(), "Wrong source fallback".into()),
        ("1".into(), "Deuxième <i0>deux</i0>".into()),
        ("0".into(), "Premier <b0>un</b0>".into()),
        ("2".into(), "Must remain locked".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Premier <b0>un</b0>".into()),
            ("1".into(), "Deuxième <i0>deux</i0>".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.tag_name().name() == "entry")
            .map(|node| {
                (
                    node.attribute("key"),
                    descendant_text(node),
                    node.children()
                        .filter(|child| child.is_element())
                        .map(|child| child.tag_name().name())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some("first"), "Premier un".into(), vec!["b"]),
            (Some("second"), "Deuxième deux".into(), vec!["i"]),
            (Some("locked"), "Locked".into(), vec![]),
        ]
    );
}

#[test]
fn schematron_intact_and_translate_false_regions_survive_strict_writeback() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("rules.sch");
    let output = temp.path().join("rules-fr.sch");
    std::fs::write(
        &source,
        r#"<schema><phase><assert>Hidden phase</assert></phase><pattern><rule><assert test="a">Outer <em>inline</em></assert><report translate="false">Locked report</report><report>Second report</report></rule></pattern></schema>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "schematron");
    let context = FilterContext::default();
    assert_eq!(filter.file_supported(&source, &context), true);
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "Outer <e0>inline</e0>".into()),
            ("1".into(), "Second report".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Deuxième rapport".into()),
        ("0".into(), "Extérieur <e0>en ligne</e0>".into()),
        ("2".into(), "Must stay locked".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Extérieur <e0>en ligne</e0>".into()),
            ("1".into(), "Deuxième rapport".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "assert" | "report"))
            .map(descendant_text)
            .collect::<Vec<_>>(),
        vec![
            "Hidden phase",
            "Extérieur en ligne",
            "Locked report",
            "Deuxième rapport",
        ]
    );
}

#[test]
fn relaxng_documentation_writes_around_intact_schema_values() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("schema.rng");
    let output = temp.path().join("schema-fr.rng");
    std::fs::write(
        &source,
        r#"<grammar xmlns="http://relaxng.org/ns/structure/1.0" xmlns:a="http://relaxng.org/ns/compatibility/annotations/1.0"><start><value>LOCKED</value></start><a:documentation>First note</a:documentation><a:documentation>Second <em>inline</em></a:documentation></grammar>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "relaxng");
    let context = FilterContext::default();
    assert_eq!(filter.file_supported(&source, &context), true);
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "First note".into()),
            ("1".into(), "Second <e0>inline</e0>".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Deuxième <e0>en ligne</e0>".into()),
        ("0".into(), "Première note".into()),
        ("LOCKED".into(), "Must not replace schema value".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Première note".into()),
            ("1".into(), "Deuxième <e0>en ligne</e0>".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "value" | "documentation"))
            .map(descendant_text)
            .collect::<Vec<_>>(),
        vec!["LOCKED", "Première note", "Deuxième en ligne"]
    );
}

#[test]
fn svg_text_keeps_style_and_path_subtrees_intact() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("diagram.svg");
    let output = temp.path().join("diagram-fr.svg");
    std::fs::write(
        &source,
        r#"<svg><style>.label{display:block}</style><text>First <tspan>label</tspan></text><path d="M0 0"><title>Hidden path</title></path><p>Second label</p></svg>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "svg");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "First <t0>label</t0>".into()),
            ("1".into(), "Second label".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Deuxième étiquette".into()),
        ("0".into(), "Première <t0>étiquette</t0>".into()),
        ("Hidden path".into(), "Must remain hidden".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Première <t0>étiquette</t0>".into()),
            ("1".into(), "Deuxième étiquette".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "style" | "text" | "title" | "p"))
            .map(descendant_text)
            .collect::<Vec<_>>(),
        vec![
            ".label{display:block}",
            "Première étiquette",
            "Hidden path",
            "Deuxième étiquette",
        ]
    );
}

#[test]
fn camtasia_paragraphs_write_without_touching_numeric_project_settings() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("project.camproj");
    let output = temp.path().join("project-fr.camproj");
    std::fs::write(
        &source,
        r#"<Project_Data><Caption>Same</Caption><AudioClickSensitivity>73</AudioClickSensitivity><RichText>Same <b>rich</b></RichText><QuestionGroup_Array>opaque</QuestionGroup_Array></Project_Data>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "camtasia");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "Same".into()),
            ("1".into(), "Same <b0>rich</b0>".into()),
        ]
    );

    let translations = HashMap::from([
        ("Same".into(), "Wrong duplicate fallback".into()),
        ("1".into(), "Deuxième <b0>riche</b0>".into()),
        ("0".into(), "Premier".into()),
        ("73".into(), "0".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Premier".into()),
            ("1".into(), "Deuxième <b0>riche</b0>".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                matches!(
                    node.tag_name().name(),
                    "Caption" | "AudioClickSensitivity" | "RichText" | "QuestionGroup_Array"
                )
            })
            .map(descendant_text)
            .collect::<Vec<_>>(),
        vec!["Premier", "73", "Deuxième riche", "opaque"]
    );
}

#[test]
fn scribus_ch_attributes_write_by_occurrence_without_touching_layout_attributes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("layout.sla");
    let output = temp.path().join("layout-fr.sla");
    std::fs::write(
        &source,
        r#"<SCRIBUSUTF8NEW Version="1.5"><DOCUMENT PAGEWIDTH="595"><ITEXT CH="Same" FONT="Alpha"/><ITEXT CH="Same" FONT="Beta"/><trail ALIGN="1"/></DOCUMENT></SCRIBUSUTF8NEW>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "scribus");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![("0".into(), "Same".into()), ("1".into(), "Same".into()),]
    );

    let translations = HashMap::from([
        ("Same".into(), "Wrong source fallback".into()),
        ("1".into(), "Deuxième".into()),
        ("0".into(), "Premier".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Premier".into()),
            ("1".into(), "Deuxième".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.tag_name().name() == "ITEXT")
            .map(|node| (node.attribute("CH"), node.attribute("FONT")))
            .collect::<Vec<_>>(),
        vec![
            (Some("Premier"), Some("Alpha")),
            (Some("Deuxième"), Some("Beta")),
        ]
    );
    let layout = document
        .descendants()
        .find(|node| node.tag_name().name() == "DOCUMENT")
        .unwrap();
    assert_eq!(layout.attribute("PAGEWIDTH"), Some("595"));
}

#[test]
fn visio_text_writeback_preserves_inline_cp_and_intact_geometry() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("drawing.vdx");
    let output = temp.path().join("drawing-fr.vdx");
    std::fs::write(
        &source,
        r#"<VisioDocument><Page><Shape><Text>First <cp IX="0"/>same</Text><Geom><Text>Hidden geometry</Text></Geom><Text>Second</Text></Shape></Page></VisioDocument>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "visio");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![
            ("0".into(), "First <c0/>same".into()),
            ("1".into(), "Second".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Deuxième".into()),
        ("0".into(), "Premier <c0/>identique".into()),
        ("Hidden geometry".into(), "Must remain hidden".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Premier <c0/>identique".into()),
            ("1".into(), "Deuxième".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    let texts = document
        .descendants()
        .filter(|node| node.tag_name().name() == "Text")
        .map(|node| {
            (
                descendant_text(node),
                node.children()
                    .filter(|child| child.is_element())
                    .map(|child| child.tag_name().name())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        texts,
        vec![
            ("Premier identique".into(), vec!["cp"]),
            ("Hidden geometry".into(), vec![]),
            ("Deuxième".into(), vec![]),
        ]
    );
}

#[test]
fn xml_spreadsheet_translation_retains_cdata_and_numeric_cells() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sheet.xml");
    let output = temp.path().join("sheet-fr.xml");
    std::fs::write(
        &source,
        r#"<Workbook xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet"><Cell><Data ss:Type="String"><![CDATA[Alpha <b>bold</b>]]></Data></Cell><Cell><Data ss:Type="Number"><![CDATA[123]]></Data></Cell></Workbook>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "xmlss");
    let context = FilterContext::default();
    let parsed = filter.parse(&source, &context).unwrap();
    assert_eq!(
        segment_pairs(&parsed),
        vec![("0".into(), "Alpha <b>bold</b>".into())]
    );

    let translations = HashMap::from([
        ("0".into(), "Bêta <i>italique</i>".into()),
        ("123".into(), "999".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![("0".into(), "Bêta <i>italique</i>".into())]
    );
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "<?xml version=\"1.0\"?>\n<Workbook xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\"><Cell><Data ss:Type=\"String\"><![CDATA[Bêta <i>italique</i>]]></Data></Cell><Cell><Data ss:Type=\"Number\"><![CDATA[123]]></Data></Cell></Workbook>"
    );
}

#[test]
fn flash_namespace_sniff_and_cdata_writeback_use_the_public_registry_path() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("movie.xml");
    let output = temp.path().join("movie-fr.xml");
    std::fs::write(
        &source,
        r#"<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/"><timeline><characters><![CDATA[Flash <b>copy</b>]]></characters><script><![CDATA[trace('hidden')]]></script></timeline></DOMDocument>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "flash");
    let context = FilterContext::default();
    assert_eq!(filter.file_supported(&source, &context), true);
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![("0".into(), "Flash <b>copy</b>".into())]
    );

    filter
        .write(
            &source,
            &output,
            &HashMap::from([("0".into(), "Éclair <i>traduit</i>".into())]),
            &context,
        )
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![("0".into(), "Éclair <i>traduit</i>".into())]
    );
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "<?xml version=\"1.0\"?>\n<DOMDocument xmlns=\"http://ns.adobe.com/xfl/2008/\"><timeline><characters><![CDATA[Éclair <i>traduit</i>]]></characters><script><![CDATA[trace('hidden')]]></script></timeline></DOMDocument>"
    );
}

#[test]
fn wordpress_namespace_sniff_writes_post_cdata_but_not_metadata_or_titles() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("export.xml");
    let output = temp.path().join("export-fr.xml");
    std::fs::write(
        &source,
        r#"<rss xmlns:wp="http://wordpress.org/export/1.2/" xmlns:content="urn:content"><channel><wp:status>publish</wp:status><description>Public description</description><item><title>Intact title</title><content:encoded><![CDATA[Post <strong>body</strong>]]></content:encoded></item></channel></rss>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "wordpress");
    let context = FilterContext::default();
    assert_eq!(filter.file_supported(&source, &context), true);
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![
            ("0".into(), "Public description".into()),
            ("1".into(), "Post <strong>body</strong>".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Article <em>traduit</em>".into()),
        ("0".into(), "Description publique".into()),
        ("Intact title".into(), "Must remain intact".into()),
        ("publish".into(), "draft".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Description publique".into()),
            ("1".into(), "Article <em>traduit</em>".into()),
        ]
    );
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "<?xml version=\"1.0\"?>\n<rss xmlns:wp=\"http://wordpress.org/export/1.2/\" xmlns:content=\"urn:content\"><channel><wp:status>publish</wp:status><description>Description publique</description><item><title>Intact title</title><content:encoded><![CDATA[Article <em>traduit</em>]]></content:encoded></item></channel></rss>"
    );
}

#[test]
fn help_and_manual_shortcuts_and_translate_false_text_write_independently() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("topic.xml");
    let output = temp.path().join("topic-fr.xml");
    std::fs::write(
        &source,
        r#"<topic><title>Visible <link>one</link></title><para><text translate="false">Hidden</text></para><para>Body</para></topic>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "helpandmanual");
    let context = FilterContext::default();
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![
            ("0".into(), "Visible <li0>one</li0>".into()),
            ("1".into(), "Body".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Corps".into()),
        ("0".into(), "Visible <li0>un</li0>".into()),
        ("Hidden".into(), "Must remain hidden".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Visible <li0>un</li0>".into()),
            ("1".into(), "Corps".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "title" | "text" | "para"))
            .map(|node| (node.tag_name().name(), descendant_text(node)))
            .collect::<Vec<_>>(),
        vec![
            ("title", "Visible un".into()),
            ("para", "Hidden".into()),
            ("text", "Hidden".into()),
            ("para", "Corps".into()),
        ]
    );
}

#[test]
fn typo3_localizable_flag_controls_writeback_and_keeps_required_closing_tags() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("typo3.xml");
    let output = temp.path().join("typo3-fr.xml");
    std::fs::write(
        &source,
        r#"<t3_tt_content><record><title localizable="1">Visible <b>title</b></title><subtitle localizable="0">Hidden subtitle</subtitle><p localizable="1">Body</p><l18n_diffsource>Diff</l18n_diffsource></record></t3_tt_content>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "typo3");
    let context = FilterContext::default();
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![
            ("0".into(), "Visible <b0>title</b0>".into()),
            ("1".into(), "Body".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Corps".into()),
        ("0".into(), "Titre <b0>visible</b0>".into()),
        ("Hidden subtitle".into(), "Must remain hidden".into()),
        ("Diff".into(), "Must remain diff".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Titre <b0>visible</b0>".into()),
            ("1".into(), "Corps".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                matches!(
                    node.tag_name().name(),
                    "title" | "subtitle" | "p" | "l18n_diffsource"
                )
            })
            .map(|node| (node.tag_name().name(), descendant_text(node)))
            .collect::<Vec<_>>(),
        vec![
            ("title", "Titre visible".into()),
            ("subtitle", "Hidden subtitle".into()),
            ("p", "Corps".into()),
            ("l18n_diffsource", "Diff".into()),
        ]
    );
}

#[test]
fn l10nmgr_paragraph_writeback_leaves_head_metadata_intact() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("l10nmgr.xml");
    let output = temp.path().join("l10nmgr-fr.xml");
    std::fs::write(
        &source,
        r#"<TYPO3L10N><head><title>Hidden header</title></head><pageGrp><data>First <b>same</b></data><p>Second</p></pageGrp></TYPO3L10N>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "l10nmgr");
    let context = FilterContext::default();
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![
            ("0".into(), "First <b0>same</b0>".into()),
            ("1".into(), "Second".into()),
        ]
    );

    let translations = HashMap::from([
        ("1".into(), "Deuxième".into()),
        ("0".into(), "Premier <b0>identique</b0>".into()),
        ("Hidden header".into(), "Must remain metadata".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Premier <b0>identique</b0>".into()),
            ("1".into(), "Deuxième".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "title" | "data" | "p"))
            .map(|node| (node.tag_name().name(), descendant_text(node)))
            .collect::<Vec<_>>(),
        vec![
            ("title", "Hidden header".into()),
            ("data", "Premier identique".into()),
            ("p", "Deuxième".into()),
        ]
    );
}

#[test]
fn infix_br_shortcut_round_trips_without_cross_paragraph_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("infix.xml");
    let output = temp.path().join("infix-fr.xml");
    std::fs::write(
        &source,
        r#"<DOC><STORY><P>Same <BR/>line</P><P>Same line</P></STORY></DOC>"#,
    )
    .unwrap();

    let registry = FilterRegistry::new();
    let filter = registry.for_path(&source).unwrap();
    assert_eq!(filter.id(), "infix");
    let context = FilterContext::default();
    assert_eq!(
        segment_pairs(&filter.parse(&source, &context).unwrap()),
        vec![
            ("0".into(), "Same <br0/>line".into()),
            ("1".into(), "Same line".into()),
        ]
    );

    let translations = HashMap::from([
        ("Same line".into(), "Wrong source fallback".into()),
        ("1".into(), "Deuxième ligne".into()),
        ("0".into(), "Première <br0/>ligne".into()),
    ]);
    filter
        .write(&source, &output, &translations, &context)
        .unwrap();
    assert_eq!(
        segment_pairs(&filter.parse(&output, &context).unwrap()),
        vec![
            ("0".into(), "Première <br0/>ligne".into()),
            ("1".into(), "Deuxième ligne".into()),
        ]
    );

    let rewritten = std::fs::read_to_string(&output).unwrap();
    let document = roxmltree::Document::parse(&rewritten).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.tag_name().name() == "P")
            .map(|node| {
                (
                    descendant_text(node),
                    node.children()
                        .filter(|child| child.is_element())
                        .map(|child| child.tag_name().name())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Première ligne".into(), vec!["BR"]),
            ("Deuxième ligne".into(), vec![]),
        ]
    );
}
