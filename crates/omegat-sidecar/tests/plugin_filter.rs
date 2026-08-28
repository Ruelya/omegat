//! Example cdylib must execute through both Filter and Marker sidecar paths.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn example_lib() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../target/debug");
    if cfg!(target_os = "windows") {
        p.push("omegat_example_plugin.dll");
    } else if cfg!(target_os = "macos") {
        p.push("libomegat_example_plugin.dylib");
    } else {
        p.push("libomegat_example_plugin.so");
    }
    if !p.exists() {
        let st = Command::new("cargo")
            .args(["build", "-p", "omegat-example-plugin"])
            .status()
            .expect("build example plugin");
        assert!(st.success());
    }
    p
}

fn rpc(
    child_in: &mut impl Write,
    child_out: &mut impl BufRead,
    id: i64,
    method: &str,
    params: Value,
) -> Value {
    let req = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    writeln!(child_in, "{req}").unwrap();
    child_in.flush().unwrap();
    let mut line = String::new();
    child_out.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

#[test]
fn filters_list_includes_example_and_parses_fixture() {
    let lib = example_lib();
    assert!(lib.exists(), "missing {}", lib.display());
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join(lib.file_name().unwrap());
    std::fs::copy(&lib, &dest).unwrap();
    std::fs::write(
        dir.path().join("omegat-plugin.toml"),
        format!(
            "id = \"example\"\nname = \"Example Filter\"\nversion = \"1.0.0\"\nplugin_type = \"filter\"\nentry = \"{}\"\n",
            dest.file_name().unwrap().to_string_lossy()
        ),
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .env("OMEGAT_PLUGINS_DIR", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let list = rpc(&mut stdin, &mut stdout, 1, "filters.list", json!({}));
    let ids: Vec<String> = list["result"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
        .collect();
    assert!(
        ids.contains(&"example".to_string()),
        "filters.list missing example: {ids:?}"
    );

    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/plugin/sample.example");
    let parsed = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "filters.parse",
        json!({"path": fixture.to_string_lossy(), "id": "example"}),
    );
    assert_eq!(parsed["result"]["id"], "example");
    assert_eq!(
        parsed["result"]["segments"][0]["source"],
        "Hello from plugin"
    );
    assert_eq!(parsed["result"]["segments"][1]["source"], "Second line");

    let markers = rpc(&mut stdin, &mut stdout, 3, "markers.list", json!({}));
    assert_eq!(
        markers["result"],
        json!([{
            "plugin_id": "example",
            "id": "example.native-marker",
            "name": "org.omegat.example.NativePluginMarker"
        }])
    );
    let marked = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "markers.query",
        json!({
            "id": "example.native-marker",
            "entry_key": {
                "file": "source/sample.example",
                "source_text": "Hello from plugin",
                "id": "0",
                "prev": "",
                "next": "Second line",
                "path": null
            },
            "source_text": "Hello from plugin",
            "translation_text": "😀 plugin and plugin",
            "is_active": true
        }),
    );
    assert_eq!(
        marked["result"],
        json!({
            "marks": [
                {
                    "start_offset": 3,
                    "end_offset": 9,
                    "painter": "native-plugin",
                    "painter_color": "#7c3aed",
                    "tooltip_text": "Example marker in source/sample.example",
                    "entry_part": "TRANSLATION"
                },
                {
                    "start_offset": 14,
                    "end_offset": 20,
                    "painter": "native-plugin",
                    "painter_color": "#7c3aed",
                    "tooltip_text": "Example marker in source/sample.example",
                    "entry_part": "TRANSLATION"
                }
            ]
        })
    );

    let crashed = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "markers.query",
        json!({
            "id": "example.native-marker",
            "entry_key": {
                "file": "source/sample.example",
                "source_text": "Hello from plugin",
                "id": "0",
                "prev": "",
                "next": "Second line",
                "path": null
            },
            "source_text": "Hello from plugin",
            "translation_text": "plugin",
            "is_active": true,
            "crash_worker": true
        }),
    );
    assert_eq!(crashed["error"]["code"], -32603);
    assert_eq!(crashed["id"], 5);

    let alive = rpc(&mut stdin, &mut stdout, 6, "sys.version", json!({}));
    assert_eq!(alive["result"]["version"], "6.2.0");
    let marked_again = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "markers.query",
        json!({
            "id": "example.native-marker",
            "entry_key": {
                "file": "source/sample.example",
                "source_text": "Hello from plugin",
                "id": "0",
                "prev": "",
                "next": "Second line",
                "path": null
            },
            "source_text": "Hello from plugin",
            "translation_text": "😀 plugin and plugin",
            "is_active": true
        }),
    );
    assert_eq!(marked_again["result"], marked["result"]);

    let _ = child.kill();
}
