use omegat_script::{parse_script_metadata, ScriptError, ScriptItem};
use serde_json::Value;
use std::path::PathBuf;

fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens/remaining")
        .join(name);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(value["exported_by"], "org.omegat.tools.ExportGoldens");
    assert_eq!(
        value["java_test"]
            .as_str()
            .and_then(|test| test.find('#'))
            .is_some(),
        true
    );
    value
}

#[test]
fn script_item_java_methods_match_exported_results() {
    let inline = golden("ScriptItemTest-testGetTextWithScriptSource.json");
    let item = ScriptItem::inline(inline["source"].as_str().unwrap());
    assert_eq!(item.text().unwrap(), inline["text"].as_str().unwrap());
    assert_eq!(item.file_name(), inline["file_name"].as_str().unwrap());

    let valid = golden("ScriptItemTest-testScanFileForDescriptionWithValidContent.json");
    let metadata = parse_script_metadata(valid["content"].as_str().unwrap()).unwrap();
    assert_eq!(metadata.name, valid["script_name"].as_str().unwrap());
    assert_eq!(metadata.description, valid["description"].as_str().unwrap());

    let invalid = golden("ScriptItemTest-testScanFileForDescriptionWithInvalidContent.json");
    assert_eq!(
        parse_script_metadata(invalid["content"].as_str().unwrap()),
        None
    );
    assert_eq!(invalid["script_name"], Value::Null);
    assert_eq!(invalid["description"], "");

    let file = golden("ScriptItemTest-testGetTextWithValidFile.json");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(file["file_name"].as_str().unwrap());
    std::fs::write(&path, file["text"].as_str().unwrap()).unwrap();
    let item = ScriptItem::from_file(&path);
    assert_eq!(item.text().unwrap(), file["text"].as_str().unwrap());
    assert_eq!(item.file_name(), file["file_name"].as_str().unwrap());

    let missing = golden("ScriptItemTest-testGetTextWithNonexistentFile.json");
    let missing_item =
        ScriptItem::from_file(dir.path().join(missing["file_name"].as_str().unwrap()));
    let missing_error = missing_item.text().unwrap_err();
    let class = match missing_error {
        ScriptError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "FileNotFoundException"
        }
        _ => "other",
    };
    assert_eq!(class, missing["error_class"].as_str().unwrap());

    let io = golden("ScriptItemTest-testGetTextWithIOException.json");
    let io_error = ScriptItem::from_file(dir.path()).text().unwrap_err();
    assert_eq!(
        matches!(io_error, ScriptError::Io(_)),
        io["io_error"].as_bool().unwrap()
    );
}
