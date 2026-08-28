//! Java `dialect_tags.json` vs each filters3 dialect — full-set `assert_eq`.

use omegat_filters::filters3::all_dialect_tag_sets;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sorted_json_strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect();
    out.sort();
    out
}

#[test]
fn p3_dialect_tags_assert_eq_java_export() {
    let path = repo_root().join("fixtures/goldens/engine/dialect_tags.json");
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        spec["exported_by"].as_str(),
        Some("org.omegat.tools.ExportGoldens")
    );
    let java = spec["dialects"].as_array().expect("dialects");
    let rust = all_dialect_tag_sets();
    assert_eq!(rust.len(), 23, "23 Filter+Dialect pairs");
    assert_eq!(java.len(), rust.len(), "Java export dialect count");

    for (j, r) in java.iter().zip(rust.iter()) {
        let id = j["id"].as_str().unwrap();
        assert_eq!(r.id, id, "dialect order");
        assert_eq!(
            r.paragraph,
            sorted_json_strings(&j["paragraph"]),
            "{id} paragraph"
        );
        assert_eq!(r.intact, sorted_json_strings(&j["intact"]), "{id} intact");
        assert_eq!(
            r.out_of_turn,
            sorted_json_strings(&j["out_of_turn"]),
            "{id} out_of_turn"
        );
        assert_eq!(
            r.preformat,
            sorted_json_strings(&j["preformat"]),
            "{id} preformat"
        );
        assert_eq!(r.attrs, sorted_json_strings(&j["attrs"]), "{id} attrs");

        let mut java_tag_attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(obj) = j["tag_attrs"].as_object() {
            for (k, v) in obj {
                java_tag_attrs.insert(k.clone(), sorted_json_strings(v));
            }
        }
        let rust_tag_attrs: BTreeMap<String, Vec<String>> = r.tag_attrs.iter().cloned().collect();
        assert_eq!(rust_tag_attrs, java_tag_attrs, "{id} tag_attrs");

        let mut java_c: BTreeMap<String, String> = BTreeMap::new();
        if let Some(obj) = j["constraints"].as_object() {
            for (k, v) in obj {
                java_c.insert(k.clone(), v.as_str().unwrap_or("").to_string());
            }
        }
        let rust_c: BTreeMap<String, String> = r.constraints.iter().cloned().collect();
        assert_eq!(rust_c, java_c, "{id} constraints");
    }
}

#[test]
fn p3_sniff_xml_unknown_is_not_android() {
    assert_eq!(
        omegat_filters::filters3::sniff_xml_id("<unknown><x>hi</x></unknown>"),
        None
    );
    assert_eq!(
        omegat_filters::filters3::sniff_xml_id("<resources><string>hi</string></resources>"),
        Some("android")
    );
}
