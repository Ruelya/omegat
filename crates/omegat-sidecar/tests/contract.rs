//! Contract tests: every exposed sidecar method has a stable request/response shape.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const METHODS: &[&str] = &[
    "sys.version",
    "sys.capabilities",
    "sys.plugins",
    "markers.list",
    "markers.query",
    "prefs.get",
    "prefs.set",
    "project.create",
    "project.open",
    "project.close",
    "project.save",
    "project.compile",
    "project.reload",
    "project.external-refresh",
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
    response_for(child_out, id)
}

fn response_for(child_out: &mut impl BufRead, id: i64) -> Value {
    loop {
        let mut line = String::new();
        child_out.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            return value;
        }
    }
}

fn notification_for(child_out: &mut impl BufRead, method: &str) -> Value {
    loop {
        let mut line = String::new();
        child_out.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        if value.get("method").and_then(Value::as_str) == Some(method) {
            return value;
        }
    }
}

fn send_cancelled_request(
    child_in: &mut impl Write,
    child_out: &mut impl BufRead,
    id: i64,
    method: &str,
    params: Value,
    started: impl FnOnce(),
) -> Value {
    writeln!(
        child_in,
        "{}",
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    )
    .unwrap();
    child_in.flush().unwrap();
    started();
    writeln!(
        child_in,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "method": "$/cancelRequest",
            "params": { "id": id }
        })
    )
    .unwrap();
    child_in.flush().unwrap();
    response_for(child_out, id)
}

fn blocking_http_endpoint() -> (String, mpsc::Receiver<()>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        accepted_tx.send(()).unwrap();
        let mut buffer = [0u8; 8192];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    (
        format!("http://{address}/v2/check"),
        accepted_rx,
        worker,
    )
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
fn cancel_notification_stops_a_long_search_and_keeps_sidecar_responsive() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("cancel-search");
    let created = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": true
        }),
    );
    assert!(created["result"].is_object(), "{created}");
    let source = (0..20_000)
        .map(|index| format!("Unique segment number {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.join("source/many.txt"), source).unwrap();
    let reloaded = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "project.reload",
        json!({}),
    );
    assert!(
        reloaded["result"]["entries"].as_u64().unwrap_or(0) > 10_000,
        "{reloaded}"
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "search.run",
            "params": {
                "query": "missing phrase",
                "source": true,
                "translation": true
            }
        })
    )
    .unwrap();
    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "method": "$/cancelRequest",
            "params": { "id": 3 }
        })
    )
    .unwrap();
    stdin.flush().unwrap();
    let mut cancelled_line = String::new();
    stdout.read_line(&mut cancelled_line).unwrap();
    let cancelled: Value = serde_json::from_str(&cancelled_line).unwrap();
    assert_eq!(cancelled["id"], 3);
    assert_eq!(cancelled["error"]["code"], -32800);
    assert_eq!(cancelled["error"]["message"], "request cancelled");

    let responsive = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "sys.version",
        json!({}),
    );
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn cancellation_reaches_languagetool_issues_and_filter_product_paths() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("cancel-products");
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
    assert!(created["result"].is_object(), "{created}");
    std::fs::write(root.join("source/input.txt"), "Source").unwrap();
    let reloaded = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "project.reload",
        json!({}),
    );
    assert_eq!(reloaded["result"]["entries"], 1);
    let listed = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}));
    let entry = &listed["result"][0];
    let updated = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.set",
        json!({
            "index": 0,
            "key": entry["key"],
            "translation": "teh target",
            "note": "",
            "revision": entry["revision"],
            "default_translation": true
        }),
    );
    assert_eq!(updated["result"]["entry"]["translation"], "teh target");

    let (lt_url, lt_started, lt_worker) = blocking_http_endpoint();
    let mut prefs = rpc(&mut stdin, &mut stdout, 5, "prefs.get", json!({}))["result"].clone();
    prefs["languagetool_url"] = json!(lt_url);
    let configured = rpc(&mut stdin, &mut stdout, 6, "prefs.set", prefs);
    assert_eq!(configured["result"]["languagetool_url"], json!(lt_url));
    let cancelled_lt = send_cancelled_request(
        &mut stdin,
        &mut stdout,
        7,
        "languagetool.check",
        json!({ "text": "teh target" }),
        || {
            lt_started
                .recv_timeout(Duration::from_secs(5))
                .expect("LanguageTool curl did not start");
        },
    );
    assert_eq!(
        cancelled_lt["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    lt_worker.join().unwrap();

    let (issues_url, issues_started, issues_worker) = blocking_http_endpoint();
    let mut prefs = rpc(&mut stdin, &mut stdout, 8, "prefs.get", json!({}))["result"].clone();
    prefs["languagetool_url"] = json!(issues_url);
    let _ = rpc(&mut stdin, &mut stdout, 9, "prefs.set", prefs);
    let cancelled_issues = send_cancelled_request(
        &mut stdin,
        &mut stdout,
        10,
        "issues.list",
        json!({}),
        || {
            issues_started
                .recv_timeout(Duration::from_secs(5))
                .expect("issues LanguageTool curl did not start");
        },
    );
    assert_eq!(
        cancelled_issues["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    issues_worker.join().unwrap();

    let large = temp.path().join("large.txt");
    let file = std::fs::File::create(&large).unwrap();
    file.set_len(256 * 1024 * 1024).unwrap();
    let cancelled_filter = send_cancelled_request(
        &mut stdin,
        &mut stdout,
        11,
        "filters.parse",
        json!({ "id": "text", "path": large }),
        || {},
    );
    assert_eq!(
        cancelled_filter["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    let responsive = rpc(
        &mut stdin,
        &mut stdout,
        12,
        "sys.version",
        json!({}),
    );
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn external_refresh_reloads_source_and_glossary_over_ndjson() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("external-refresh");
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
    let glossary = created["result"]["glossary_file"].as_str().unwrap();
    std::fs::write(root.join("source/input.txt"), "Before").unwrap();
    let _ = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "project.reload",
        json!({}),
    );
    std::fs::write(root.join("source/input.txt"), "After term").unwrap();
    std::fs::write(glossary, "term\tterme\texternal\n").unwrap();

    let refreshed = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "project.external-refresh",
        json!({}),
    );
    assert_eq!(refreshed["result"]["entries"], 1);
    let entries = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.list",
        json!({}),
    );
    assert_eq!(entries["result"][0]["source"], "After term");
    let glossary_hits = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "glossary.query",
        json!({ "index": 0 }),
    );
    assert_eq!(
        glossary_hits["result"],
        json!([{"source": "term", "target": "terme", "comment": "external"}])
    );
    let _ = child.kill();
}

#[test]
fn sidecar_proactively_reports_files_created_in_runtime_directories() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("active-events");
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

    let nested = root.join("source/runtime/new");
    std::fs::create_dir_all(&nested).unwrap();
    let source = nested.join("chapter.txt");
    std::fs::write(&source, "Proactive source").unwrap();
    let event = notification_for(&mut stdout, "project.files-changed");
    assert_eq!(
        event["params"],
        json!({
            "root": root.to_string_lossy(),
            "paths": [source.to_string_lossy()]
        })
    );

    let refreshed = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "project.external-refresh",
        json!({}),
    );
    assert_eq!(refreshed["result"]["entries"], 1);
    let entries = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}));
    assert_eq!(entries["result"][0]["source"], "Proactive source");
    assert_eq!(entries["result"][0]["file"], "runtime/new/chapter.txt");
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
    let reloaded = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
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
