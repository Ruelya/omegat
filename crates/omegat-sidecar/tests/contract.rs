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

    let _ = child.kill();
}
