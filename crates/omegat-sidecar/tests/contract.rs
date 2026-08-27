//! Contract tests: every exposed sidecar method has a stable request/response shape.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const METHODS: &[&str] = &[
    "sys.version",
    "sys.capabilities",
    "sys.plugins",
    "prefs.get",
    "prefs.set",
    "project.create",
    "project.open",
    "project.close",
    "project.save",
    "project.compile",
    "project.reload",
    "project.props",
    "entry.list",
    "entry.get",
    "entry.set",
    "matches.query",
    "glossary.query",
    "glossary.add",
    "search.run",
    "search.replace",
    "stats.get",
    "issues.list",
    "filters.list",
    "filters.options",
    "filters.parse",
    "script.slots",
    "mt.query",
    "dict.query",
    "completer.query",
    "spell.check",
    "spell.learn",
    "spell.ignore",
    "spell.install",
    "tmx.export",
    "languagetool.check",
    "finder.run",
    "team.sync",
    "team.commit",
    "team.conflicts",
    "team.resolve",
    "team.mapping",
    "project.update",
    "script.run",
    "align.run",
    "align.edit",
    "align.write",
    "aligner.configure",
    "wiki.import",
    "med.open",
    "project.convert",
    "project.import",
    "script.slot",
];

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
fn every_listed_method_is_known() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let ver = rpc(&mut stdin, &mut stdout, 1, "sys.version", json!({}));
    assert!(ver["result"]["version"].is_string());
    assert_eq!(ver["result"]["rewrite"], true);

    let caps = rpc(&mut stdin, &mut stdout, 2, "sys.capabilities", json!({}));
    assert!(caps["result"]["filters"].is_array());

    for (i, method) in METHODS.iter().enumerate() {
        let resp = rpc(&mut stdin, &mut stdout, 100 + i as i64, method, json!({}));
        let err_code = resp["error"]["code"].as_i64();
        assert_ne!(
            err_code,
            Some(-32601),
            "{method} must not be METHOD_NOT_FOUND"
        );
    }

    let unknown = rpc(&mut stdin, &mut stdout, 999, "no.such.method", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);

    let _ = child.kill();
}

#[test]
fn editor_commit_propagates_defaults_and_scopes_alternatives_over_ndjson() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("editor-translations");

    let created = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false
        }),
    );
    assert_eq!(created["result"]["root"], root.to_string_lossy().as_ref());
    std::fs::write(root.join("source/a.txt"), "Repeated").unwrap();
    std::fs::write(root.join("source/b.txt"), "Repeated").unwrap();
    let reloaded = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "project.reload",
        json!({}),
    );
    assert_eq!(reloaded["result"]["entries"], 2);
    let listed = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}));
    assert_eq!(
        listed["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (
                entry["index"].as_u64().unwrap(),
                entry["file"].as_str().unwrap(),
                entry["source"].as_str().unwrap(),
                entry["revision"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![(0, "a.txt", "Repeated", 1), (1, "b.txt", "Repeated", 1)]
    );
    let first_key = listed["result"][0]["key"].clone();
    let second_key = listed["result"][1]["key"].clone();

    let shared = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.set",
        json!({
            "index": 0,
            "key": first_key,
            "translation": "Partagé",
            "note": "default note",
            "revision": 1,
            "default_translation": true
        }),
    );
    assert_eq!(
        shared["result"]["updated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (
                entry["index"].as_u64().unwrap(),
                entry["translation"].as_str().unwrap(),
                entry["note"].as_str().unwrap(),
                entry["default_translation"].as_bool().unwrap(),
                entry["revision"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "Partagé", "default note", true, 2),
            (1, "Partagé", "default note", true, 2),
        ]
    );

    let alternative = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "entry.set",
        json!({
            "index": 1,
            "key": second_key,
            "translation": "Occurrence privée",
            "note": "alternative note",
            "revision": 2,
            "default_translation": false
        }),
    );
    assert_eq!(
        alternative["result"]["updated"],
        json!([{
            "index": 1,
            "key": {
                "file": "b.txt",
                "source_text": "Repeated",
                "id": "0",
                "prev": "",
                "next": "",
                "path": null
            },
            "file": "b.txt",
            "id": "0",
            "source": "Repeated",
            "translation": "Occurrence privée",
            "note": "alternative note",
            "comment": "",
            "default_translation": false,
            "revision": 3,
            "translated": true,
            "tags": [],
            "properties": [
                ["changeid", "omegat-rewrite"],
                ["changedate", alternative["result"]["entry"]["properties"][1][1].clone()]
            ]
        }])
    );
    let final_entries = rpc(&mut stdin, &mut stdout, 6, "entry.list", json!({}));
    assert_eq!(
        final_entries["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (
                entry["index"].as_u64().unwrap(),
                entry["translation"].as_str().unwrap(),
                entry["default_translation"].as_bool().unwrap(),
                entry["revision"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![(0, "Partagé", true, 2), (1, "Occurrence privée", false, 3)]
    );

    let stale = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "entry.set",
        json!({
            "index": 0,
            "translation": "Stale",
            "revision": 1,
            "default_translation": true
        }),
    );
    assert_eq!(stale["error"]["code"], -32002);
    assert_eq!(
        stale["error"]["message"],
        "optimistic lock failed for entry 0"
    );

    let misspelled = rpc(
        &mut stdin,
        &mut stdout,
        8,
        "spell.check",
        json!({"text": "😀 bonjour xyzzyqq"}),
    );
    assert_eq!(
        misspelled["result"],
        json!([{"word": "xyzzyqq", "offset": 11, "length": 7}])
    );
    let learned = rpc(
        &mut stdin,
        &mut stdout,
        9,
        "spell.learn",
        json!({"word": "xyzzyqq"}),
    );
    assert_eq!(learned["result"], json!({"ok": true}));
    let rechecked = rpc(
        &mut stdin,
        &mut stdout,
        10,
        "spell.check",
        json!({"text": "😀 bonjour xyzzyqq"}),
    );
    assert_eq!(rechecked["result"], json!([]));
    let _ = child.kill();
}

#[test]
fn alignment_manual_edit_is_written_through_rpc() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let pairs = json!([
        {"source":"one","target":"un"},
        {"source":"two","target":"deux"}
    ]);
    let edited = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "align.edit",
        json!({"action":"merge","side":"source","index":0,"pairs":pairs}),
    );
    assert_eq!(
        edited["result"]["pairs"],
        json!([
            {"source":"one two","target":"un"},
            {"source":"","target":"deux"}
        ])
    );

    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("manual.tmx");
    let written = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "align.write",
        json!({
            "dest": dest,
            "source_lang": "en",
            "target_lang": "fr",
            "pairs": edited["result"]["pairs"]
        }),
    );
    assert_eq!(written["result"]["ok"], true);
    assert_eq!(written["result"]["count"], 2);
    let parsed = omegat_core::tmx::parse_tmx(&std::fs::read_to_string(dest).unwrap(), "en", "fr");
    let actual: Vec<_> = parsed
        .entries
        .into_iter()
        .map(|entry| (entry.source, entry.translation))
        .collect();
    assert_eq!(
        actual,
        vec![
            ("one two".to_string(), "un".to_string()),
            (String::new(), "deux".to_string())
        ]
    );
    let _ = child.kill();
}

#[test]
fn alignment_mutable_beads_preserve_review_pinpoint_and_multiline_state() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let beads = json!([
        {"source":"one","target":"un","source_lines":["one"],"target_lines":["un"],"enabled":true},
        {"source":"two words","target":"deux mots","source_lines":["two words"],"target_lines":["deux","mots"],"enabled":true},
        {"source":"three","target":"trois","source_lines":["three"],"target_lines":["trois"],"enabled":true}
    ]);

    let split = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "align.edit",
        json!({
            "action":"split",
            "side":"source",
            "index":1,
            "line_index":0,
            "lines":["two","words"],
            "beads":beads
        }),
    );
    assert_eq!(
        split["result"]["beads"][1]["source_lines"],
        json!(["two", "words"])
    );
    assert_eq!(split["result"]["beads"][1]["status"], "default");

    let review = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "align.edit",
        json!({
            "action":"needs-review",
            "index":1,
            "beads":split["result"]["beads"]
        }),
    );
    assert_eq!(review["result"]["beads"][1]["status"], "needs-review");
    assert_eq!(
        review["result"]["selection"],
        json!({"anchor_row":3,"focus_row":3})
    );

    let toggled = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "align.edit",
        json!({
            "action":"toggle-keep",
            "indexes":[0,1,1],
            "beads":review["result"]["beads"]
        }),
    );
    assert_eq!(
        toggled["result"]["beads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bead| bead["enabled"].as_bool().unwrap())
            .collect::<Vec<_>>(),
        vec![false, false, true]
    );

    let merged_span = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "align.edit",
        json!({
            "action":"merge",
            "side":"target",
            "start_row":1,
            "end_row":2,
            "source_lang":"en",
            "target_lang":"fr",
            "beads":toggled["result"]["beads"]
        }),
    );
    assert_eq!(
        merged_span["result"]["beads"][1]["target_lines"],
        json!(["deux mots"])
    );
    assert_eq!(merged_span["result"]["row_count"], 4);

    let replaced_span = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "align.edit",
        json!({
            "action":"replace-span",
            "side":"source",
            "start_row":1,
            "end_row":2,
            "lines":["two revised","words revised"],
            "beads":merged_span["result"]["beads"]
        }),
    );
    assert_eq!(
        replaced_span["result"]["beads"][1]["source_lines"],
        json!(["two revised", "words revised"])
    );

    let pinpoint = rpc(
        &mut stdin,
        &mut stdout,
        6,
        "align.edit",
        json!({
            "action":"pinpoint",
            "side":"source",
            "start_row":0,
            "end_row":2,
            "end_side":"target",
            "beads":replaced_span["result"]["beads"]
        }),
    );
    assert_eq!(
        pinpoint["result"]["beads"][1]["source_lines"],
        json!(["one", "two revised", "words revised"])
    );
    assert_eq!(pinpoint["result"]["beads"][1]["status"], "accepted");
    assert_eq!(
        pinpoint["result"]["pairs"],
        json!([
            {"source":"","target":"un"},
            {"source":"one two revised words revised","target":"deux mots"},
            {"source":"three","target":"trois"}
        ])
    );

    let dropped = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "align.edit",
        json!({
            "action":"move-to-row",
            "side":"source",
            "start_row":1,
            "end_row":2,
            "target_row":4,
            "beads":[
                {"source":"a b","target":"A","source_lines":["a","b"],"target_lines":["A"],"score":1,"status":"accepted","enabled":true},
                {"source":"c","target":"C D","source_lines":["c"],"target_lines":["C","D"],"score":2,"status":"needs-review","enabled":true},
                {"source":"e","target":"E","source_lines":["e"],"target_lines":["E"],"score":3,"status":"accepted","enabled":true}
            ]
        }),
    );
    assert_eq!(
        dropped["result"]["beads"],
        json!([
            {"source":"a","target":"A","source_lines":["a"],"target_lines":["A"],"score":1.0,"status":"default","enabled":true},
            {"source":"","target":"C D","source_lines":[],"target_lines":["C","D"],"score":2.0,"status":"default","enabled":true},
            {"source":"c b e","target":"E","source_lines":["c","b","e"],"target_lines":["E"],"score":3.0,"status":"default","enabled":true}
        ])
    );
    assert_eq!(dropped["result"]["row_count"], 6);
    assert_eq!(
        dropped["result"]["selection"],
        json!({"anchor_row":4,"focus_row":3})
    );

    let _ = child.kill();
}
