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
    "project.recovery.detach",
    "project.save",
    "project.compile",
    "project.reload",
    "project.external-refresh",
    "project.refresh.enqueue",
    "project.refresh.discard",
    "transaction.receipt.discover",
    "transaction.receipt.pending",
    "transaction.receipt.ack",
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

fn cancel_at_checkpoint(
    child_in: &mut impl Write,
    child_out: &mut impl BufRead,
    id: i64,
    method: &str,
    mut params: Value,
    stage: &str,
) -> Value {
    let progress_token = format!("{method}-{id}");
    params
        .as_object_mut()
        .expect("checkpoint request params must be an object")
        .insert("progress_token".into(), json!(progress_token));
    writeln!(
        child_in,
        "{}",
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    )
    .unwrap();
    child_in.flush().unwrap();
    loop {
        let mut line = String::new();
        child_out.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_ne!(
            value.get("id").and_then(Value::as_i64),
            Some(id),
            "{method} completed before reaching checkpoint {stage}: {value}"
        );
        if value.get("method").and_then(Value::as_str) == Some("$/progress")
            && value["params"]["token"] == progress_token
            && value["params"]["stage"] == stage
        {
            break;
        }
    }
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

fn file_snapshot(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &std::path::Path, path: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                collect(root, &path, out);
            } else if file_type.is_file() {
                out.push((
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn copy_product_tree(from: &std::path::Path, to: &std::path::Path) {
    for (relative, bytes) in file_snapshot(from) {
        if relative == ".repositories" || relative.starts_with(".repositories/") {
            continue;
        }
        let destination = to.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(destination, bytes).unwrap();
    }
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
    (format!("http://{address}/v2/check"), accepted_rx, worker)
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
fn editor_set_save_and_close_share_durable_product_receipts() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("product-receipts");
    rpc(
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
    std::fs::write(root.join("source/first.txt"), "Repeated source").unwrap();
    std::fs::write(root.join("source/second.txt"), "Repeated source").unwrap();
    rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
    let listed = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}));
    let entries = listed["result"].as_array().unwrap();
    let wanted = entries
        .iter()
        .find(|entry| entry["key"]["file"] == "second.txt")
        .unwrap();
    assert_eq!(
        wanted["key"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        ["file", "id", "next", "path", "prev", "source_text"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
    let set = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.set",
        json!({
            "index": wanted["index"],
            "key": wanted["key"],
            "translation": "Occurrence durable",
            "note": "receipt-bound",
            "revision": wanted["revision"],
            "default_translation": false,
            "transaction_project_root": root,
            "transaction_generation": 23,
            "transaction_batch_id": "editor-set-23"
        }),
    );
    assert_eq!(set["result"]["entry"]["key"], wanted["key"]);
    assert_eq!(set["result"]["entry"]["translation"], "Occurrence durable");
    assert_eq!(set["result"]["receipt"]["status"], "sidecar_committed");
    assert_eq!(
        set["result"]["receipt"]["payload"]["operation"],
        "entry.set"
    );
    let set_ack = rpc(
        &mut stdin,
        &mut stdout,
        41,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-editor",
            "generation": 23,
            "batch_id": "editor-set-23",
            "operation": "entry.set",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(set_ack["result"]["ack"]["acknowledged"], true);

    let history_path = root.join(".repositories/transactions/history.ndjson");
    let history = std::fs::read_to_string(&history_path).unwrap();
    let committed_set = history
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["batch_id"] == "editor-set-23")
        .last()
        .unwrap();
    assert_eq!(committed_set["version"], 1);
    assert_eq!(committed_set["generation"], 23);
    assert_eq!(committed_set["status"], "completed");
    assert_eq!(committed_set["payload"]["operation"], "entry.set");
    assert_eq!(
        committed_set["commit"]["manifest_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(committed_set["payload"]["product_manifest"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "project/omegat/project_save.tmx"));
    let key: omegat_ipc::EntryKeyDto = serde_json::from_value(wanted["key"].clone()).unwrap();
    let saved =
        omegat_core::tmx::ProjectTmx::load(&root.join("omegat/project_save.tmx"), "en", "fr")
            .unwrap();
    assert_eq!(
        saved
            .get_multiple_translation_for_key(&key)
            .unwrap()
            .translation,
        "Occurrence durable"
    );

    let save = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "project.save",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 23,
            "transaction_batch_id": "document-save-23"
        }),
    );
    assert_eq!(save["result"]["ok"], true);
    assert_eq!(
        save["result"]["receipt"]["payload"]["operation"],
        "project.save"
    );
    let save_ack = rpc(
        &mut stdin,
        &mut stdout,
        51,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-editor",
            "generation": 23,
            "batch_id": "document-save-23",
            "operation": "project.save",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(save_ack["result"]["ack"]["acknowledged"], true);
    let staged = rpc(
        &mut stdin,
        &mut stdout,
        6,
        "script.run",
        json!({
            "index": wanted["index"],
            "source": "editor.setTranslation('Close durable');"
        }),
    );
    assert_eq!(staged["result"]["translation"], "Close durable");
    assert_eq!(staged["result"]["saved"], false);
    let close = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "project.close",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 23,
            "transaction_batch_id": "project-close-23"
        }),
    );
    assert_eq!(close["result"]["ok"], true);
    assert_eq!(
        close["result"]["receipt"]["payload"]["operation"],
        "project.close"
    );
    let close_ack = rpc(
        &mut stdin,
        &mut stdout,
        71,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-editor",
            "generation": 23,
            "batch_id": "project-close-23",
            "operation": "project.close",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(close_ack["result"]["ack"]["acknowledged"], true);

    let history = std::fs::read_to_string(history_path).unwrap();
    for (batch, operation) in [
        ("document-save-23", "project.save"),
        ("project-close-23", "project.close"),
    ] {
        let committed = history
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["batch_id"] == batch)
            .last()
            .unwrap();
        assert_eq!(committed["status"], "completed");
        assert_eq!(committed["payload"]["operation"], operation);
        assert!(committed["commit"].is_object());
    }
    let closed =
        omegat_core::tmx::ProjectTmx::load(&root.join("omegat/project_save.tmx"), "en", "fr")
            .unwrap();
    assert_eq!(
        closed
            .get_default_translation("Repeated source")
            .unwrap()
            .translation,
        "Close durable"
    );
    assert!(!root.join(".repositories/transactions/active.json").exists());
    let _ = child.kill();
}

#[test]
fn remaining_project_writes_publish_and_ack_exact_receipts() {
    fn scoped(root: &std::path::Path, batch: &str, mut params: Value) -> Value {
        let object = params.as_object_mut().unwrap();
        object.insert("transaction_project_root".into(), json!(root));
        object.insert("transaction_generation".into(), json!(81));
        object.insert("transaction_batch_id".into(), json!(batch));
        params
    }

    fn assert_and_ack(
        stdin: &mut impl Write,
        stdout: &mut impl BufRead,
        id: i64,
        root: &std::path::Path,
        batch: &str,
        operation: &str,
        response: &Value,
    ) {
        assert_eq!(response["result"]["receipt"]["batch_id"], batch);
        assert_eq!(
            response["result"]["receipt"]["payload"]["operation"],
            operation
        );
        assert_eq!(response["result"]["receipt"]["status"], "sidecar_committed");
        let acknowledged = rpc(
            stdin,
            stdout,
            id,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": "remaining-writes-contract",
                "generation": 81,
                "batch_id": batch,
                "operation": operation,
                "outcome": "succeeded",
            }),
        );
        assert_eq!(acknowledged["result"]["ack"]["acknowledged"], true);
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("project");
    let wiki_source = temp.path().join("wiki.txt");
    let exported_tmx = temp.path().join("exported.tmx");
    std::fs::write(&wiki_source, "Imported wiki text").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .env("OMEGAT_CONFIG_DIR", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    std::fs::write(root.join("source/source.txt"), "Receipt source").unwrap();
    rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
    let entry = rpc(&mut stdin, &mut stdout, 3, "entry.get", json!({"index": 0}));
    rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.set",
        json!({
            "index": 0,
            "key": entry["result"]["key"],
            "translation": "alpha",
            "note": "",
            "revision": entry["result"]["revision"],
            "default_translation": true,
        }),
    );

    let glossary = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "glossary.add",
        scoped(
            &root,
            "glossary-receipt",
            json!({"source": "cat", "target": "chat", "comment": "term"}),
        ),
    );
    assert_and_ack(
        &mut stdin,
        &mut stdout,
        6,
        &root,
        "glossary-receipt",
        "glossary.add",
        &glossary,
    );
    assert_eq!(
        std::fs::read_to_string(root.join("glossary/glossary.txt")).unwrap(),
        "cat\tchat\tterm\n"
    );

    let replaced = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "search.replace",
        scoped(
            &root,
            "replace-receipt",
            json!({
                "query": "alpha",
                "replace": "beta",
                "source": false,
                "translation": true,
            }),
        ),
    );
    assert_eq!(replaced["result"]["replaced"], 1);
    assert_eq!(
        rpc(&mut stdin, &mut stdout, 8, "entry.get", json!({"index": 0}),)["result"]["translation"],
        "beta"
    );
    assert_and_ack(
        &mut stdin,
        &mut stdout,
        9,
        &root,
        "replace-receipt",
        "search.replace",
        &replaced,
    );

    for (request_id, ack_id, method, batch, word) in [
        (10, 11, "spell.ignore", "ignore-receipt", "ignoredword"),
        (12, 13, "spell.learn", "learn-receipt", "learnedword"),
    ] {
        let response = rpc(
            &mut stdin,
            &mut stdout,
            request_id,
            method,
            scoped(&root, batch, json!({"word": word})),
        );
        assert_and_ack(
            &mut stdin,
            &mut stdout,
            ack_id,
            &root,
            batch,
            method,
            &response,
        );
    }
    assert!(root.join("omegat/ignored_words.txt").is_file());
    assert!(root.join("omegat/learned_words.txt").is_file());

    let exported = rpc(
        &mut stdin,
        &mut stdout,
        14,
        "tmx.export",
        scoped(
            &root,
            "tmx-export-receipt",
            json!({"dest": exported_tmx, "level": "level2"}),
        ),
    );
    assert_and_ack(
        &mut stdin,
        &mut stdout,
        15,
        &root,
        "tmx-export-receipt",
        "tmx.export",
        &exported,
    );
    assert_eq!(
        std::fs::read_to_string(&exported_tmx).unwrap(),
        exported["result"]["xml"].as_str().unwrap()
    );

    let wiki = rpc(
        &mut stdin,
        &mut stdout,
        16,
        "wiki.import",
        scoped(&root, "wiki-import-receipt", json!({"source": wiki_source})),
    );
    assert_eq!(wiki["result"]["files"], 1);
    assert_and_ack(
        &mut stdin,
        &mut stdout,
        17,
        &root,
        "wiki-import-receipt",
        "wiki.import",
        &wiki,
    );
    assert!(root.join("source/wiki.txt").is_file());

    let script = rpc(
        &mut stdin,
        &mut stdout,
        18,
        "script.run",
        scoped(
            &root,
            "script-run-receipt",
            json!({
                "index": 0,
                "source": "editor.setTranslation('Script durable'); project.save(); glossary.addEntry('dog','chien','script');",
            }),
        ),
    );
    assert_eq!(script["result"]["saved"], true);
    assert_and_ack(
        &mut stdin,
        &mut stdout,
        19,
        &root,
        "script-run-receipt",
        "script.run",
        &script,
    );
    let saved =
        omegat_core::tmx::ProjectTmx::load(&root.join("omegat/project_save.tmx"), "en", "fr")
            .unwrap();
    assert_eq!(
        saved
            .get_default_translation("Receipt source")
            .unwrap()
            .translation,
        "Script durable"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("glossary/glossary.txt")).unwrap(),
        "cat\tchat\tterm\ndog\tchien\tscript\n"
    );

    assert!(!root.join(".repositories/transactions/active.json").exists());
    let history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    for batch in [
        "glossary-receipt",
        "replace-receipt",
        "ignore-receipt",
        "learn-receipt",
        "tmx-export-receipt",
        "wiki-import-receipt",
        "script-run-receipt",
    ] {
        assert_eq!(
            history
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|row| row["batch_id"] == batch && row["status"] == "completed")
                .count(),
            1,
            "{batch}"
        );
    }
    let _ = child.kill();
}

#[test]
fn global_prefs_and_spell_install_stay_outside_project_journal() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("project");
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .env("OMEGAT_CONFIG_DIR", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    let mut preferences = rpc(&mut stdin, &mut stdout, 2, "prefs.get", json!({}))["result"].clone();
    preferences["filter_options"]["text"] = json!({"preserve_spaces": "global"});
    preferences["srx_path"] = json!("global-rules.srx");
    let saved = rpc(&mut stdin, &mut stdout, 3, "prefs.set", preferences);
    assert_eq!(
        saved["result"]["filter_options"]["text"]["preserve_spaces"],
        "global"
    );
    assert_eq!(saved["result"]["srx_path"], "global-rules.srx");
    assert!(config.join("omegat.prefs.json").is_file());
    assert!(!root.join(".repositories/transactions/active.json").exists());

    let installed = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "spell.install",
        json!({"lang": "en"}),
    );
    assert_eq!(installed["result"]["ok"], true);
    assert!(config.join("spell/hunspell/en.aff").is_file());
    assert!(config.join("spell/hunspell/en.dic").is_file());
    assert!(!root.join(".repositories/transactions/active.json").exists());

    std::fs::remove_dir_all(config.join("spell/hunspell")).unwrap();
    std::fs::write(config.join("spell/hunspell"), "not a directory").unwrap();
    let failed_spell = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "spell.install",
        json!({"lang": "en"}),
    );
    assert_eq!(failed_spell["error"]["code"], -32003);
    assert!(!root.join(".repositories/transactions/active.json").exists());

    let persisted = rpc(&mut stdin, &mut stdout, 6, "prefs.get", json!({}))["result"].clone();
    std::fs::remove_file(config.join("omegat.prefs.json")).unwrap();
    std::fs::create_dir(config.join("omegat.prefs.json")).unwrap();
    let mut rejected = persisted.clone();
    rejected["theme"] = json!("dark");
    let failed = rpc(&mut stdin, &mut stdout, 7, "prefs.set", rejected);
    assert_eq!(failed["error"]["code"], -32003);
    let after = rpc(&mut stdin, &mut stdout, 8, "prefs.get", json!({}));
    assert_eq!(after["result"]["theme"], persisted["theme"]);
    assert_eq!(
        after["result"]["filter_options"]["text"]["preserve_spaces"],
        "global"
    );
    assert!(!root.join(".repositories/transactions/active.json").exists());
    let _ = child.kill();
}

#[test]
fn close_receipt_is_discovered_and_acknowledged_without_an_open_project() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("closed-project");
    let (mut first, mut first_in, mut first_out) = spawn_sidecar(&config);
    rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    std::fs::write(root.join("source/source.txt"), "close discovery source").unwrap();
    rpc(
        &mut first_in,
        &mut first_out,
        2,
        "project.reload",
        json!({}),
    );
    let closed = rpc(
        &mut first_in,
        &mut first_out,
        3,
        "project.close",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 9,
            "transaction_batch_id": "detached-close-9",
        }),
    );
    assert_eq!(
        closed["result"]["receipt"]["payload"]["operation"],
        "project.close"
    );
    let selected = rpc(
        &mut first_in,
        &mut first_out,
        4,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "closed-electron",
            "generation": 9,
        }),
    );
    assert_eq!(
        selected["result"]["envelopes"][0]["batch_id"],
        "detached-close-9"
    );
    first.kill().unwrap();
    first.wait().unwrap();

    let active_path = root.join(".repositories/transactions/active.json");
    let active_before_discovery = std::fs::read(&active_path).unwrap();
    let (mut replacement, mut replacement_in, mut replacement_out) = spawn_sidecar(&config);
    let discovered = rpc(
        &mut replacement_in,
        &mut replacement_out,
        5,
        "transaction.receipt.discover",
        json!({}),
    );
    assert_eq!(
        discovered["result"]["projects"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        discovered["result"]["projects"][0]["project_root"],
        root.to_string_lossy().as_ref()
    );
    assert_eq!(
        discovered["result"]["projects"][0]["batch_id"],
        "detached-close-9"
    );
    assert_eq!(
        std::fs::read(&active_path).unwrap(),
        active_before_discovery,
        "discovery adopted the receipt before selecting its exact root"
    );

    let adopted = rpc(
        &mut replacement_in,
        &mut replacement_out,
        6,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "replacement-electron",
            "generation": 10,
        }),
    );
    assert_eq!(adopted["result"]["envelopes"][0]["generation"], 10);
    assert_eq!(
        adopted["result"]["envelopes"][0]["payload"]["operation"],
        "project.close"
    );
    let acknowledged = rpc(
        &mut replacement_in,
        &mut replacement_out,
        7,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "replacement-electron",
            "generation": 10,
            "batch_id": "detached-close-9",
            "operation": "project.close",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(acknowledged["result"]["ack"]["acknowledged"], true);
    let after = rpc(
        &mut replacement_in,
        &mut replacement_out,
        8,
        "transaction.receipt.discover",
        json!({}),
    );
    assert_eq!(after["result"]["projects"], json!([]));
    assert!(!active_path.exists());
    let history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    assert_eq!(
        history
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|row| {
                row["batch_id"] == "detached-close-9"
                    && row["status"] == "completed"
                    && row["payload"]["phase"] == "renderer-acknowledged"
            })
            .count(),
        1
    );
    replacement.kill().unwrap();
}

#[test]
fn close_team_and_save_receipts_queue_and_one_live_replacement_owns_dispatch() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        (child, input, output)
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("queued-project");
    let remote = temp.path().join("queued-remote");
    let active = root.join(".repositories/transactions/active.json");
    let history = root.join(".repositories/transactions/history.ndjson");
    std::fs::create_dir_all(remote.join("target")).unwrap();
    std::fs::write(remote.join("target/queued.txt"), "remote-before").unwrap();
    let (mut first, mut first_in, mut first_out) = spawn_sidecar(&config);
    rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    let mapped = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/target/queued.txt",
                    "repository": "/target/queued.txt",
                    "includes": [],
                    "excludes": [],
                }],
            }],
        }),
    );
    assert_eq!(mapped["result"]["ok"], true);
    let synced = rpc(&mut first_in, &mut first_out, 3, "team.sync", json!({}));
    assert_eq!(synced["result"]["action"], "sync");
    std::fs::write(root.join("source/source.txt"), "queued source").unwrap();
    rpc(
        &mut first_in,
        &mut first_out,
        4,
        "project.reload",
        json!({}),
    );
    let closed = rpc(
        &mut first_in,
        &mut first_out,
        5,
        "project.close",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 51,
            "transaction_batch_id": "queued-close",
        }),
    );
    assert_eq!(
        closed["result"]["receipt"]["payload"]["operation"],
        "project.close"
    );
    let reopened = rpc(
        &mut first_in,
        &mut first_out,
        6,
        "project.open",
        json!({ "root": root }),
    );
    assert_eq!(reopened["error"], Value::Null);
    std::fs::write(root.join("target/queued.txt"), "committed exactly once").unwrap();
    let team = rpc(
        &mut first_in,
        &mut first_out,
        7,
        "team.commit",
        json!({
            "which": "target",
            "transaction_project_root": root,
            "transaction_generation": 51,
            "transaction_batch_id": "queued-team",
        }),
    );
    assert_eq!(
        team["result"]["receipt"]["payload"]["operation"],
        "commit-target"
    );
    let saved = rpc(
        &mut first_in,
        &mut first_out,
        8,
        "project.save",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 51,
            "transaction_batch_id": "queued-save",
        }),
    );
    assert_eq!(
        saved["result"]["receipt"]["batch_id"], "queued-save",
        "direct reply returned the older close head instead of the new tail"
    );
    let journal: Value = serde_json::from_slice(&std::fs::read(&active).unwrap()).unwrap();
    assert_eq!(journal["version"], 2);
    assert_eq!(
        journal["batches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["batch_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["queued-close", "queued-team", "queued-save"]
    );
    assert!(journal["batches"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["status"] == "sidecar_committed"));

    let selected = rpc(
        &mut first_in,
        &mut first_out,
        9,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "original-electron",
            "generation": 51,
        }),
    );
    assert_eq!(
        selected["result"]["envelopes"][0]["batch_id"],
        "queued-close"
    );
    first.kill().unwrap();
    first.wait().unwrap();

    let save_tmx = root.join("omegat/project_save.tmx");
    let product_before_recovery = std::fs::read(&save_tmx).unwrap();
    let product_mtime_before_recovery = std::fs::metadata(&save_tmx).unwrap().modified().unwrap();
    let remote_before_recovery = std::fs::read(remote.join("target/queued.txt")).unwrap();
    let remote_mtime_before_recovery = std::fs::metadata(remote.join("target/queued.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let (mut owner, mut owner_in, mut owner_out) = spawn_sidecar(&config);
    let owner_selected = rpc(
        &mut owner_in,
        &mut owner_out,
        10,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
        }),
    );
    assert_eq!(
        owner_selected["result"]["envelopes"][0]["batch_id"],
        "queued-close"
    );
    assert_eq!(owner_selected["result"]["envelopes"][0]["generation"], 52);

    let (mut contender, mut contender_in, mut contender_out) = spawn_sidecar(&config);
    let rejected = rpc(
        &mut contender_in,
        &mut contender_out,
        11,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "replacement-contender",
            "generation": 53,
        }),
    );
    assert_eq!(rejected["error"]["code"], -32005);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("owned by live app"));
    let rejected_ack = rpc(
        &mut contender_in,
        &mut contender_out,
        12,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "replacement-contender",
            "generation": 53,
            "batch_id": "queued-close",
            "operation": "project.close",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(rejected_ack["error"]["code"], -32005);
    assert_eq!(std::fs::read(&save_tmx).unwrap(), product_before_recovery);
    assert_eq!(
        std::fs::metadata(&save_tmx).unwrap().modified().unwrap(),
        product_mtime_before_recovery
    );

    let close_ack = rpc(
        &mut owner_in,
        &mut owner_out,
        13,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
            "batch_id": "queued-close",
            "operation": "project.close",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(close_ack["result"]["ack"]["acknowledged"], true);
    let team_head = rpc(
        &mut owner_in,
        &mut owner_out,
        14,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
        }),
    );
    assert_eq!(
        team_head["result"]["envelopes"][0]["batch_id"],
        "queued-team"
    );
    let team_ack = rpc(
        &mut owner_in,
        &mut owner_out,
        15,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
            "batch_id": "queued-team",
            "operation": "commit-target",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(team_ack["result"]["ack"]["acknowledged"], true);
    let save_head = rpc(
        &mut owner_in,
        &mut owner_out,
        16,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
        }),
    );
    assert_eq!(
        save_head["result"]["envelopes"][0]["batch_id"],
        "queued-save"
    );
    let save_ack = rpc(
        &mut owner_in,
        &mut owner_out,
        17,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "replacement-owner",
            "generation": 52,
            "batch_id": "queued-save",
            "operation": "project.save",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(save_ack["result"]["ack"]["acknowledged"], true);
    assert!(!active.exists());
    let rows = std::fs::read_to_string(history).unwrap();
    for batch_id in ["queued-close", "queued-team", "queued-save"] {
        assert_eq!(
            rows.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|row| {
                    row["batch_id"] == batch_id
                        && row["status"] == "completed"
                        && row["payload"]["phase"] == "renderer-acknowledged"
                })
                .count(),
            1
        );
    }
    assert_eq!(std::fs::read(&save_tmx).unwrap(), product_before_recovery);
    assert_eq!(
        std::fs::metadata(&save_tmx).unwrap().modified().unwrap(),
        product_mtime_before_recovery
    );
    assert_eq!(
        std::fs::read(remote.join("target/queued.txt")).unwrap(),
        remote_before_recovery
    );
    assert_eq!(
        std::fs::metadata(remote.join("target/queued.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        remote_mtime_before_recovery
    );

    contender.kill().unwrap();
    contender.wait().unwrap();
    owner.kill().unwrap();
    owner.wait().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn dead_owner_product_heads_choose_one_of_simultaneous_replacements() {
    struct Sidecar {
        child: std::process::Child,
        input: std::process::ChildStdin,
        output: BufReader<std::process::ChildStdout>,
    }

    fn spawn_sidecar(config: &std::path::Path) -> Sidecar {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Sidecar {
            child,
            input,
            output,
        }
    }

    fn pending(
        sidecar: &mut Sidecar,
        id: i64,
        root: &std::path::Path,
        app_instance: &str,
        generation: u64,
    ) -> Value {
        rpc(
            &mut sidecar.input,
            &mut sidecar.output,
            id,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": app_instance,
                "generation": generation,
            }),
        )
    }

    for (kind, operation) in [
        ("close", "project.close"),
        ("team", "commit-target"),
        ("save", "project.save"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join(format!("{kind}-config"));
        let root = temp.path().join(format!("{kind}-project"));
        let remote = temp.path().join(format!("{kind}-remote"));
        let source_path = root.join("source/source.txt");
        let head_batch = format!("atomic-{kind}-head");
        let owner_path = root.join(".repositories/transactions/renderer-owner.json");
        let active_path = root.join(".repositories/transactions/active.json");
        let history_path = root.join(".repositories/transactions/history.ndjson");

        let mut setup = spawn_sidecar(&config);
        rpc(
            &mut setup.input,
            &mut setup.output,
            1,
            "project.create",
            json!({
                "root": root,
                "source_lang": "en",
                "target_lang": "fr",
                "sentence_seg": false,
            }),
        );
        std::fs::write(&source_path, format!("atomic {kind} source")).unwrap();
        rpc(
            &mut setup.input,
            &mut setup.output,
            2,
            "project.reload",
            json!({}),
        );
        let entries = rpc(
            &mut setup.input,
            &mut setup.output,
            20,
            "entry.list",
            json!({}),
        );
        let entry = &entries["result"][0];
        let mut key_fields = entry["key"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        key_fields.sort_unstable();
        assert_eq!(
            key_fields,
            ["file", "id", "next", "path", "prev", "source_text"]
        );
        let initial = rpc(
            &mut setup.input,
            &mut setup.output,
            21,
            "entry.set",
            json!({
                "index": entry["index"],
                "key": entry["key"],
                "translation": format!("atomic {kind} translation"),
                "note": "atomic replacement election",
                "revision": entry["revision"],
                "default_translation": false,
            }),
        );
        assert_eq!(initial["result"]["receipt"], Value::Null);

        let remote_path = remote.join("target/atomic.txt");
        if kind == "team" {
            std::fs::create_dir_all(remote_path.parent().unwrap()).unwrap();
            std::fs::write(&remote_path, "remote before atomic election").unwrap();
            let mapped = rpc(
                &mut setup.input,
                &mut setup.output,
                3,
                "team.mapping",
                json!({
                    "repositories": [{
                        "repo_type": "file",
                        "url": remote,
                        "branch": null,
                        "mappings": [{
                            "local": "/target/atomic.txt",
                            "repository": "/target/atomic.txt",
                            "includes": [],
                            "excludes": [],
                        }],
                    }],
                }),
            );
            assert_eq!(mapped["result"]["ok"], true);
            let synced = rpc(
                &mut setup.input,
                &mut setup.output,
                4,
                "team.sync",
                json!({}),
            );
            assert_eq!(synced["result"]["action"], "sync");
            std::fs::write(
                root.join("target/atomic.txt"),
                "remote committed exactly once",
            )
            .unwrap();
        }

        let head = match kind {
            "close" => rpc(
                &mut setup.input,
                &mut setup.output,
                5,
                "project.close",
                json!({
                    "transaction_project_root": root,
                    "transaction_generation": 61,
                    "transaction_batch_id": head_batch,
                }),
            ),
            "team" => rpc(
                &mut setup.input,
                &mut setup.output,
                5,
                "team.commit",
                json!({
                    "which": "target",
                    "transaction_project_root": root,
                    "transaction_generation": 61,
                    "transaction_batch_id": head_batch,
                }),
            ),
            "save" => rpc(
                &mut setup.input,
                &mut setup.output,
                5,
                "project.save",
                json!({
                    "transaction_project_root": root,
                    "transaction_generation": 61,
                    "transaction_batch_id": head_batch,
                }),
            ),
            _ => unreachable!(),
        };
        assert_eq!(head["result"]["receipt"]["batch_id"], head_batch);
        assert_eq!(head["result"]["receipt"]["payload"]["operation"], operation);
        assert_eq!(head["result"]["receipt"]["status"], "sidecar_committed");

        if kind == "close" {
            let reopened = rpc(
                &mut setup.input,
                &mut setup.output,
                6,
                "project.open",
                json!({ "root": root }),
            );
            assert_eq!(reopened["error"], Value::Null);
        }
        std::thread::sleep(Duration::from_millis(10));
        let refresh = rpc(
            &mut setup.input,
            &mut setup.output,
            7,
            "project.refresh.enqueue",
            json!({
                "root": root,
                "app_instance": format!("atomic-{kind}-setup"),
                "generation": 61,
                "paths": [source_path],
                "fingerprints": {
                    format!("source-{kind}"): format!("atomic-{kind}-refresh")
                },
                "sources": ["native"],
            }),
        );
        let refresh_batch = refresh["result"]["batch"]["batch_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(refresh["result"]["batch"]["status"], "pending");

        let selected_by_old_owner = pending(
            &mut setup,
            8,
            &root,
            &format!("atomic-{kind}-old-owner"),
            62,
        );
        assert_eq!(
            selected_by_old_owner["result"]["envelopes"][0]["batch_id"],
            head_batch
        );
        let old_owner: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(old_owner["process_id"], setup.child.id());
        assert_eq!(
            old_owner["app_instance"],
            format!("atomic-{kind}-old-owner")
        );

        let mut live_contender = spawn_sidecar(&config);
        let rejected_while_old_owner_lived = pending(
            &mut live_contender,
            9,
            &root,
            &format!("atomic-{kind}-pre-kill-contender"),
            63,
        );
        assert_eq!(rejected_while_old_owner_lived["error"]["code"], -32005);
        assert_eq!(rejected_while_old_owner_lived["result"], Value::Null);
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&owner_path).unwrap()).unwrap(),
            old_owner,
            "{kind} pre-kill contender replaced the live owner"
        );
        live_contender.child.kill().unwrap();
        live_contender.child.wait().unwrap();

        let tmx_path = root.join("omegat/project_save.tmx");
        let tmx_before = std::fs::read(&tmx_path).unwrap();
        let tmx_mtime_before = std::fs::metadata(&tmx_path).unwrap().modified().unwrap();
        let remote_before = (kind == "team").then(|| std::fs::read(&remote_path).unwrap());
        let remote_mtime_before =
            (kind == "team").then(|| std::fs::metadata(&remote_path).unwrap().modified().unwrap());

        let old_pid = setup.child.id();
        setup.child.kill().unwrap();
        assert!(!setup.child.wait().unwrap().success());
        assert!(
            !std::path::Path::new("/proc")
                .join(old_pid.to_string())
                .exists(),
            "{kind} old owner PID still existed before replacement race"
        );

        let replacements = [
            (
                spawn_sidecar(&config),
                format!("atomic-{kind}-replacement-a"),
                64,
            ),
            (
                spawn_sidecar(&config),
                format!("atomic-{kind}-replacement-b"),
                65,
            ),
        ];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut racers = Vec::new();
        for (mut sidecar, app_instance, generation) in replacements {
            let barrier = barrier.clone();
            let root = root.clone();
            racers.push(std::thread::spawn(move || {
                barrier.wait();
                let response = pending(&mut sidecar, 10, &root, &app_instance, generation);
                (sidecar, app_instance, generation, response)
            }));
        }
        barrier.wait();
        let mut raced = racers
            .into_iter()
            .map(|racer| racer.join().unwrap())
            .collect::<Vec<_>>();

        let winner_index = raced
            .iter()
            .position(|(_, _, _, response)| {
                response
                    .pointer("/result/envelopes/0/batch_id")
                    .and_then(Value::as_str)
                    == Some(head_batch.as_str())
            })
            .expect("one replacement must receive the durable product head");
        assert_eq!(
            raced
                .iter()
                .filter(|(_, _, _, response)| {
                    response
                        .pointer("/result/envelopes/0/batch_id")
                        .and_then(Value::as_str)
                        == Some(head_batch.as_str())
                })
                .count(),
            1,
            "{kind} product head was delivered to more than one replacement"
        );
        let loser_index = 1 - winner_index;
        assert_eq!(raced[loser_index].3["error"]["code"], -32005);
        assert_eq!(raced[loser_index].3["result"], Value::Null);

        let replacement_owner: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(replacement_owner["app_instance"], raced[winner_index].1);
        assert_eq!(
            replacement_owner["process_id"],
            raced[winner_index].0.child.id()
        );
        assert_eq!(replacement_owner["generation"], raced[winner_index].2);
        assert_ne!(replacement_owner["claim_id"], old_owner["claim_id"]);
        assert!(replacement_owner["claim_id"]
            .as_str()
            .is_some_and(|claim| !claim.is_empty()));

        let (winner, loser) = if winner_index == 0 {
            let loser = raced.pop().unwrap();
            let winner = raced.pop().unwrap();
            (winner, loser)
        } else {
            let winner = raced.pop().unwrap();
            let loser = raced.pop().unwrap();
            (winner, loser)
        };
        let (mut winner, winner_app, winner_generation, _) = winner;
        let (mut loser, loser_app, loser_generation, _) = loser;

        let loser_ack = rpc(
            &mut loser.input,
            &mut loser.output,
            11,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": loser_app,
                "generation": loser_generation,
                "batch_id": head_batch,
                "operation": operation,
                "outcome": "succeeded",
            }),
        );
        assert_eq!(loser_ack["error"]["code"], -32005);
        assert_eq!(loser_ack["result"], Value::Null);

        let winner_ack = rpc(
            &mut winner.input,
            &mut winner.output,
            12,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": winner_app,
                "generation": winner_generation,
                "batch_id": head_batch,
                "operation": operation,
                "outcome": "succeeded",
            }),
        );
        assert_eq!(winner_ack["result"]["ack"]["acknowledged"], true);
        let winner_refresh = pending(&mut winner, 13, &root, &winner_app, winner_generation);
        assert_eq!(
            winner_refresh["result"]["envelopes"][0]["batch_id"],
            refresh_batch
        );
        assert_eq!(
            winner_refresh["result"]["envelopes"][0]["payload"]["operation"],
            "project.external-refresh"
        );
        let refresh_ack = rpc(
            &mut winner.input,
            &mut winner.output,
            14,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": winner_app,
                "generation": winner_generation,
                "batch_id": refresh_batch,
                "operation": "project.external-refresh",
                "outcome": "coalesced",
            }),
        );
        assert_eq!(refresh_ack["result"]["ack"]["acknowledged"], true);
        assert_eq!(
            pending(&mut winner, 15, &root, &winner_app, winner_generation,)["result"]["envelopes"],
            json!([])
        );
        assert!(!active_path.exists());

        let product_history = std::fs::read_to_string(&history_path).unwrap();
        assert_eq!(
            product_history
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| {
                    row["batch_id"] == head_batch
                        && row["status"] == "completed"
                        && row["payload"]["phase"] == "renderer-acknowledged"
                })
                .count(),
            1,
            "{kind} head had duplicate terminal acknowledgements"
        );
        assert_eq!(
            product_history
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| { row["batch_id"] == refresh_batch && row["status"] == "completed" })
                .count(),
            1,
            "{kind} refresh tail had duplicate terminal acknowledgements"
        );
        assert_eq!(std::fs::read(&tmx_path).unwrap(), tmx_before);
        assert_eq!(
            std::fs::metadata(&tmx_path).unwrap().modified().unwrap(),
            tmx_mtime_before,
            "{kind} election replayed the committed TMX write"
        );
        if let (Some(remote_before), Some(remote_mtime_before)) =
            (remote_before, remote_mtime_before)
        {
            assert_eq!(std::fs::read(&remote_path).unwrap(), remote_before);
            assert_eq!(
                std::fs::metadata(&remote_path).unwrap().modified().unwrap(),
                remote_mtime_before,
                "team election replayed the committed remote write"
            );
        }

        loser.child.kill().unwrap();
        loser.child.wait().unwrap();
        winner.child.kill().unwrap();
        winner.child.wait().unwrap();
    }
}

#[test]
fn team_renderer_receipt_ack_survives_sidecar_restart_and_is_idempotent() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("team-receipt-project");
    let remote = temp.path().join("team-receipt-remote");
    std::fs::create_dir_all(remote.join("source")).unwrap();
    std::fs::write(remote.join("source/shared.txt"), "remote-before").unwrap();

    let (mut first_child, mut first_in, mut first_out) = spawn_sidecar(&config);
    let created = rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    assert_eq!(created["result"]["root"], root.to_string_lossy().as_ref());
    let mapped = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/source/shared.txt",
                    "repository": "/source/shared.txt",
                    "includes": [],
                    "excludes": [],
                }],
            }],
        }),
    );
    assert_eq!(mapped["result"]["ok"], true);
    let initialized = rpc(&mut first_in, &mut first_out, 3, "team.sync", json!({}));
    assert_eq!(initialized["result"]["action"], "sync");
    std::fs::write(root.join("source/shared.txt"), "renderer-ack-candidate").unwrap();
    let committed = rpc(
        &mut first_in,
        &mut first_out,
        4,
        "team.commit",
        json!({
            "which": "source",
            "transaction_project_root": root,
            "transaction_generation": 11,
            "transaction_batch_id": "renderer-team-ack",
        }),
    );
    assert_eq!(committed["error"], Value::Null);
    let receipt = &committed["result"]["receipt"];
    assert_eq!(receipt["version"], 1);
    assert_eq!(receipt["batch_id"], "renderer-team-ack");
    assert_eq!(receipt["generation"], 11);
    assert_eq!(receipt["status"], "sidecar_committed");
    assert_eq!(receipt["payload"]["operation"], "commit-source");
    assert_eq!(
        std::fs::read_to_string(remote.join("source/shared.txt")).unwrap(),
        "renderer-ack-candidate"
    );
    let active = root.join(".repositories/transactions/active.json");
    assert!(active.exists());
    first_child.kill().unwrap();
    first_child.wait().unwrap();

    let (mut second_child, mut second_in, mut second_out) = spawn_sidecar(&config);
    let opened = rpc(
        &mut second_in,
        &mut second_out,
        5,
        "project.open",
        json!({ "root": root }),
    );
    assert_eq!(opened["error"], Value::Null);
    let pending = rpc(
        &mut second_in,
        &mut second_out,
        6,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "contract-team",
            "generation": 12,
        }),
    );
    assert_eq!(
        pending["result"]["envelopes"][0]["batch_id"],
        "renderer-team-ack"
    );
    assert_eq!(pending["result"]["envelopes"][0]["generation"], 12);
    assert_eq!(
        pending["result"]["envelopes"][0]["status"],
        "sidecar_committed"
    );
    let acknowledged = rpc(
        &mut second_in,
        &mut second_out,
        7,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-team",
            "generation": 12,
            "batch_id": "renderer-team-ack",
            "operation": "commit-source",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(acknowledged["result"]["ack"]["acknowledged"], true);
    assert_eq!(acknowledged["result"]["ack"]["already_acknowledged"], false);
    assert!(!active.exists());
    let history_path = root.join(".repositories/transactions/history.ndjson");
    let history_after_ack = std::fs::read(&history_path).unwrap();
    let remote_after_ack = file_snapshot(&remote);
    second_child.kill().unwrap();
    second_child.wait().unwrap();

    let (mut third_child, mut third_in, mut third_out) = spawn_sidecar(&config);
    rpc(
        &mut third_in,
        &mut third_out,
        8,
        "project.open",
        json!({ "root": root }),
    );
    let duplicate = rpc(
        &mut third_in,
        &mut third_out,
        9,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-team",
            "generation": 12,
            "batch_id": "renderer-team-ack",
            "operation": "commit-source",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(duplicate["result"]["ack"]["acknowledged"], true);
    assert_eq!(duplicate["result"]["ack"]["already_acknowledged"], true);
    assert_eq!(std::fs::read(&history_path).unwrap(), history_after_ack);
    assert_eq!(file_snapshot(&remote), remote_after_ack);
    let no_pending = rpc(
        &mut third_in,
        &mut third_out,
        10,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "contract-team",
            "generation": 12,
        }),
    );
    assert_eq!(no_pending["result"]["envelopes"], json!([]));
    let unknown = rpc(
        &mut third_in,
        &mut third_out,
        11,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "contract-team",
            "generation": 12,
            "batch_id": "unknown-receipt",
            "operation": "commit-source",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(unknown["error"]["code"], -32005);
    let _ = third_child.kill();
}

#[test]
fn concurrent_project_recoveries_isolate_product_and_refresh_receipts() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    fn create_project(
        child_in: &mut impl Write,
        child_out: &mut impl BufRead,
        id: i64,
        root: &std::path::Path,
        source: &str,
    ) -> Value {
        rpc(
            child_in,
            child_out,
            id,
            "project.create",
            json!({
                "root": root,
                "source_lang": "en",
                "target_lang": "fr",
                "sentence_seg": false,
            }),
        );
        std::fs::write(root.join("source/source.txt"), source).unwrap();
        rpc(child_in, child_out, id + 1, "project.reload", json!({}));
        rpc(child_in, child_out, id + 2, "entry.list", json!({}))["result"][0].clone()
    }

    let temp = tempfile::tempdir().unwrap();
    let root_a = temp.path().join("project-a");
    let root_b = temp.path().join("project-b");
    let shared_config = temp.path().join("shared-config");
    let (mut first_a, mut first_a_in, mut first_a_out) = spawn_sidecar(&shared_config);
    let (mut first_b, mut first_b_in, mut first_b_out) = spawn_sidecar(&shared_config);

    let entry_a = create_project(
        &mut first_a_in,
        &mut first_a_out,
        1,
        &root_a,
        "project A source",
    );
    let committed_a = rpc(
        &mut first_a_in,
        &mut first_a_out,
        4,
        "entry.set",
        json!({
            "index": entry_a["index"],
            "key": entry_a["key"],
            "translation": "project A committed",
            "note": "concurrent recovery",
            "revision": entry_a["revision"],
            "default_translation": false,
            "transaction_project_root": root_a,
            "transaction_generation": 7,
            "transaction_batch_id": "concurrent-product-a",
        }),
    );
    assert_eq!(
        committed_a["result"]["receipt"]["payload"]["operation"],
        "entry.set"
    );

    create_project(
        &mut first_b_in,
        &mut first_b_out,
        11,
        &root_b,
        "project B before refresh",
    );
    std::fs::write(root_b.join("source/source.txt"), "project B after refresh").unwrap();
    let queued_b = rpc(
        &mut first_b_in,
        &mut first_b_out,
        14,
        "project.refresh.enqueue",
        json!({
            "root": root_b,
            "app_instance": "project-b-before-kill",
            "generation": 8,
            "paths": [root_b.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "project-b-after-refresh" },
            "sources": ["native"],
        }),
    );
    let batch_b = queued_b["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    let committed_b = rpc(
        &mut first_b_in,
        &mut first_b_out,
        15,
        "project.external-refresh",
        json!({
            "transaction_project_root": root_b,
            "transaction_generation": 8,
            "transaction_batch_id": batch_b,
            "app_instance": "project-b-before-kill",
        }),
    );
    assert_eq!(committed_b["error"], Value::Null);

    first_a.kill().unwrap();
    first_b.kill().unwrap();
    first_a.wait().unwrap();
    first_b.wait().unwrap();

    let (mut recovered_a, mut recovered_a_in, mut recovered_a_out) = spawn_sidecar(&shared_config);
    let (mut recovered_b, mut recovered_b_in, mut recovered_b_out) = spawn_sidecar(&shared_config);
    rpc(
        &mut recovered_a_in,
        &mut recovered_a_out,
        101,
        "project.open",
        json!({ "root": root_a }),
    );
    rpc(
        &mut recovered_b_in,
        &mut recovered_b_out,
        201,
        "project.open",
        json!({ "root": root_b }),
    );

    let pending_a = rpc(
        &mut recovered_a_in,
        &mut recovered_a_out,
        102,
        "transaction.receipt.pending",
        json!({
            "root": root_a,
            "app_instance": "project-a-after-kill",
            "generation": 101,
        }),
    );
    let pending_b = rpc(
        &mut recovered_b_in,
        &mut recovered_b_out,
        202,
        "transaction.receipt.pending",
        json!({
            "root": root_b,
            "app_instance": "project-b-after-kill",
            "generation": 202,
        }),
    );
    assert_eq!(
        pending_a["result"]["envelopes"][0]["batch_id"],
        "concurrent-product-a"
    );
    assert_eq!(
        pending_a["result"]["envelopes"][0]["project_root"],
        root_a.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(pending_a["result"]["envelopes"][0]["generation"], 101);
    assert_eq!(
        pending_a["result"]["envelopes"][0]["payload"]["operation"],
        "entry.set"
    );
    assert_eq!(
        pending_b["result"]["envelopes"][0]["batch_id"],
        batch_b.as_str()
    );
    assert_eq!(
        pending_b["result"]["envelopes"][0]["project_root"],
        root_b.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(pending_b["result"]["envelopes"][0]["generation"], 202);
    assert_eq!(
        pending_b["result"]["envelopes"][0]["payload"]["operation"],
        "project.external-refresh"
    );

    let cross_root = rpc(
        &mut recovered_a_in,
        &mut recovered_a_out,
        103,
        "transaction.receipt.pending",
        json!({
            "root": root_b,
            "app_instance": "project-a-after-kill",
            "generation": 101,
        }),
    );
    assert_eq!(cross_root["error"]["code"], -32602);
    let still_a = rpc(
        &mut recovered_a_in,
        &mut recovered_a_out,
        104,
        "transaction.receipt.pending",
        json!({
            "root": root_a,
            "app_instance": "project-a-after-kill",
            "generation": 101,
        }),
    );
    assert_eq!(
        still_a["result"]["envelopes"][0]["batch_id"],
        "concurrent-product-a"
    );

    let ack_a = rpc(
        &mut recovered_a_in,
        &mut recovered_a_out,
        105,
        "transaction.receipt.ack",
        json!({
            "root": root_a,
            "app_instance": "project-a-after-kill",
            "generation": 101,
            "batch_id": "concurrent-product-a",
            "operation": "entry.set",
            "outcome": "succeeded",
        }),
    );
    let ack_b = rpc(
        &mut recovered_b_in,
        &mut recovered_b_out,
        203,
        "transaction.receipt.ack",
        json!({
            "root": root_b,
            "app_instance": "project-b-after-kill",
            "generation": 202,
            "batch_id": batch_b,
            "operation": "project.external-refresh",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(ack_a["result"]["ack"]["acknowledged"], true);
    assert_eq!(ack_b["result"]["ack"]["acknowledged"], true);
    assert!(!root_a
        .join(".repositories/transactions/active.json")
        .exists());
    assert!(!root_b
        .join(".repositories/transactions/external-refresh.json")
        .exists());

    recovered_a.kill().unwrap();
    recovered_b.kill().unwrap();
    recovered_a.wait().unwrap();
    recovered_b.wait().unwrap();
}

#[test]
fn team_refresh_and_save_receipts_share_one_stable_fifo_dispatch() {
    fn pending(
        input: &mut impl Write,
        output: &mut impl BufRead,
        id: i64,
        root: &std::path::Path,
    ) -> Value {
        rpc(
            input,
            output,
            id,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": "fair-dispatch-electron",
                "generation": 31,
            }),
        )
    }

    fn acknowledge(
        input: &mut impl Write,
        output: &mut impl BufRead,
        id: i64,
        root: &std::path::Path,
        batch_id: &str,
        operation: &str,
        outcome: &str,
    ) -> Value {
        rpc(
            input,
            output,
            id,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": "fair-dispatch-electron",
                "generation": 31,
                "batch_id": batch_id,
                "operation": operation,
                "outcome": outcome,
            }),
        )
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("project");
    let remote = temp.path().join("remote");
    std::fs::create_dir_all(remote.join("source")).unwrap();
    std::fs::write(remote.join("source/shared.txt"), "remote initial").unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .env("OMEGAT_CONFIG_DIR", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    rpc(
        &mut input,
        &mut output,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    let mapped = rpc(
        &mut input,
        &mut output,
        2,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/source/shared.txt",
                    "repository": "/source/shared.txt",
                    "includes": [],
                    "excludes": [],
                }],
            }],
        }),
    );
    assert_eq!(mapped["result"]["ok"], true);
    let initialized = rpc(&mut input, &mut output, 3, "team.sync", json!({}));
    assert_eq!(initialized["result"]["action"], "sync");

    let old_refresh = rpc(
        &mut input,
        &mut output,
        4,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "fair-dispatch-electron",
            "generation": 31,
            "paths": [root.join("source/shared.txt")],
            "fingerprints": { "source/shared.txt": "refresh-before-team" },
            "sources": ["native"],
        }),
    );
    let old_refresh_id = old_refresh["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    std::thread::sleep(Duration::from_millis(5));
    std::fs::write(root.join("source/shared.txt"), "team committed").unwrap();
    let team = rpc(
        &mut input,
        &mut output,
        5,
        "team.commit",
        json!({
            "which": "source",
            "transaction_project_root": root,
            "transaction_generation": 31,
            "transaction_batch_id": "fair-team-receipt",
        }),
    );
    assert_eq!(
        team["result"]["receipt"]["payload"]["operation"],
        "commit-source"
    );

    let first = pending(&mut input, &mut output, 6, &root);
    assert_eq!(first["result"]["envelopes"].as_array().unwrap().len(), 1);
    assert_eq!(first["result"]["envelopes"][0]["batch_id"], old_refresh_id);
    assert_eq!(
        first["result"]["envelopes"][0]["payload"]["fingerprints"]["source/shared.txt"],
        "refresh-before-team"
    );
    assert_eq!(
        first["result"]["envelopes"][0]["payload"].get("phase"),
        None
    );
    let premature_team_ack = acknowledge(
        &mut input,
        &mut output,
        7,
        &root,
        "fair-team-receipt",
        "commit-source",
        "succeeded",
    );
    assert_eq!(premature_team_ack["error"]["code"], -32005);
    assert!(root.join(".repositories/transactions/active.json").exists());

    let old_refresh_ack = acknowledge(
        &mut input,
        &mut output,
        8,
        &root,
        &old_refresh_id,
        "project.external-refresh",
        "coalesced",
    );
    assert_eq!(old_refresh_ack["result"]["ack"]["acknowledged"], true);
    let team_head = pending(&mut input, &mut output, 9, &root);
    assert_eq!(
        team_head["result"]["envelopes"][0]["batch_id"],
        "fair-team-receipt"
    );
    assert_eq!(
        team_head["result"]["envelopes"][0]["payload"],
        json!({ "operation": "commit-source" })
    );
    assert_eq!(team_head["result"]["envelopes"][0]["generation"], 31);
    let team_ack = acknowledge(
        &mut input,
        &mut output,
        10,
        &root,
        "fair-team-receipt",
        "commit-source",
        "succeeded",
    );
    assert_eq!(team_ack["result"]["ack"]["acknowledged"], true);
    assert_eq!(
        std::fs::read_to_string(remote.join("source/shared.txt")).unwrap(),
        "team committed"
    );

    std::thread::sleep(Duration::from_millis(5));
    let refresh_one = rpc(
        &mut input,
        &mut output,
        11,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "fair-dispatch-electron",
            "generation": 31,
            "paths": [root.join("source/shared.txt")],
            "fingerprints": { "source/shared.txt": "refresh-one" },
            "sources": ["sidecar"],
        }),
    );
    let refresh_one_id = refresh_one["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    std::thread::sleep(Duration::from_millis(5));
    let refresh_two = rpc(
        &mut input,
        &mut output,
        12,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "fair-dispatch-electron",
            "generation": 31,
            "paths": [root.join("source/shared.txt")],
            "fingerprints": { "source/shared.txt": "refresh-two" },
            "sources": ["native"],
        }),
    );
    let refresh_two_id = refresh_two["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    std::thread::sleep(Duration::from_millis(5));
    let saved = rpc(
        &mut input,
        &mut output,
        13,
        "project.save",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 31,
            "transaction_batch_id": "fair-save-receipt",
        }),
    );
    assert_eq!(
        saved["result"]["receipt"]["payload"]["operation"],
        "project.save"
    );

    for (id, batch_id, operation) in [
        (14, refresh_one_id.as_str(), "project.external-refresh"),
        (18, refresh_two_id.as_str(), "project.external-refresh"),
        (22, "fair-save-receipt", "project.save"),
    ] {
        let head = pending(&mut input, &mut output, id, &root);
        assert_eq!(head["result"]["envelopes"].as_array().unwrap().len(), 1);
        assert_eq!(head["result"]["envelopes"][0]["batch_id"], batch_id);
        assert_eq!(
            head["result"]["envelopes"][0]["payload"]["operation"],
            operation
        );
        if operation == "project.external-refresh" {
            assert!(head["result"]["envelopes"][0]["payload"]["fingerprints"].is_object());
        } else {
            assert_eq!(
                head["result"]["envelopes"][0]["payload"],
                json!({ "operation": "project.save" })
            );
        }
        let ack_id = if operation == "project.external-refresh" {
            let committed = rpc(
                &mut input,
                &mut output,
                id + 1,
                "project.external-refresh",
                json!({
                    "transaction_project_root": root,
                    "transaction_generation": 31,
                    "transaction_batch_id": batch_id,
                    "app_instance": "fair-dispatch-electron",
                }),
            );
            assert_eq!(committed["error"], Value::Null);
            let committed_head = pending(&mut input, &mut output, id + 2, &root);
            assert_eq!(
                committed_head["result"]["envelopes"][0]["batch_id"], batch_id,
                "sidecar commit moved the refresh FIFO head"
            );
            assert_eq!(
                committed_head["result"]["envelopes"][0]["status"],
                "sidecar_committed"
            );
            id + 3
        } else {
            id + 1
        };
        let ack = acknowledge(
            &mut input,
            &mut output,
            ack_id,
            &root,
            batch_id,
            operation,
            "succeeded",
        );
        assert_eq!(ack["result"]["ack"]["acknowledged"], true);
    }
    let drained = pending(&mut input, &mut output, 24, &root);
    assert_eq!(drained["result"]["envelopes"], json!([]));
    assert!(!root.join(".repositories/transactions/active.json").exists());
    assert!(!root
        .join(".repositories/transactions/external-refresh.json")
        .exists());

    let _ = child.kill();
    child.wait().unwrap();
}

#[test]
fn selected_global_head_survives_sidecar_kill_before_renderer_ack() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        (child, input, output)
    }

    fn pending(
        input: &mut impl Write,
        output: &mut impl BufRead,
        id: i64,
        root: &std::path::Path,
        app_instance: &str,
        generation: u64,
    ) -> Value {
        rpc(
            input,
            output,
            id,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": app_instance,
                "generation": generation,
            }),
        )
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("project");
    let remote = temp.path().join("remote");
    std::fs::create_dir_all(remote.join("source")).unwrap();
    std::fs::write(remote.join("source/shared.txt"), "remote initial").unwrap();

    let (mut first, mut first_in, mut first_out) = spawn_sidecar(&config);
    let created = rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    assert_eq!(created["error"], Value::Null);
    let mapped = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/source/shared.txt",
                    "repository": "/source/shared.txt",
                    "includes": [],
                    "excludes": [],
                }],
            }],
        }),
    );
    assert_eq!(mapped["result"]["ok"], true);
    let synced = rpc(&mut first_in, &mut first_out, 3, "team.sync", json!({}));
    assert_eq!(synced["result"]["action"], "sync");
    std::fs::write(root.join("source/shared.txt"), "committed exactly once").unwrap();
    let committed = rpc(
        &mut first_in,
        &mut first_out,
        4,
        "team.commit",
        json!({
            "which": "source",
            "transaction_project_root": root,
            "transaction_generation": 41,
            "transaction_batch_id": "selected-team-head",
        }),
    );
    assert_eq!(committed["error"], Value::Null);
    assert_eq!(
        committed["result"]["receipt"]["payload"]["operation"],
        "commit-source"
    );
    std::thread::sleep(Duration::from_millis(5));
    let refresh = rpc(
        &mut first_in,
        &mut first_out,
        5,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-before-head-kill",
            "generation": 41,
            "paths": [root.join("source/shared.txt")],
            "fingerprints": { "source/shared.txt": "tail-after-selected-head" },
            "sources": ["native"],
        }),
    );
    let refresh_id = refresh["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    let remote_after_commit = file_snapshot(&remote);

    let selected = pending(
        &mut first_in,
        &mut first_out,
        6,
        &root,
        "electron-before-head-kill",
        41,
    );
    assert_eq!(selected["result"]["envelopes"].as_array().unwrap().len(), 1);
    assert_eq!(
        selected["result"]["envelopes"][0]["batch_id"],
        "selected-team-head"
    );
    assert_eq!(
        selected["result"]["envelopes"][0]["status"],
        "sidecar_committed"
    );
    // This is the main-process boundary: the global head response has been
    // received, but no renderer acknowledgement is sent before SIGKILL.
    first.kill().unwrap();
    first.wait().unwrap();

    let (mut second, mut second_in, mut second_out) = spawn_sidecar(&config);
    let opened = rpc(
        &mut second_in,
        &mut second_out,
        7,
        "project.open",
        json!({ "root": root }),
    );
    assert_eq!(opened["error"], Value::Null);
    let recovered = pending(
        &mut second_in,
        &mut second_out,
        8,
        &root,
        "electron-after-head-kill",
        42,
    );
    assert_eq!(
        recovered["result"]["envelopes"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        recovered["result"]["envelopes"][0]["batch_id"],
        "selected-team-head"
    );
    assert_eq!(recovered["result"]["envelopes"][0]["generation"], 42);
    assert_eq!(file_snapshot(&remote), remote_after_commit);

    let still_head = pending(
        &mut second_in,
        &mut second_out,
        9,
        &root,
        "electron-after-head-kill",
        42,
    );
    assert_eq!(
        still_head["result"]["envelopes"][0]["batch_id"],
        "selected-team-head"
    );
    let shared_journal: Value = serde_json::from_slice(
        &std::fs::read(root.join(".repositories/transactions/active.json")).unwrap(),
    )
    .unwrap();
    let refresh_row = shared_journal["batches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["batch_id"] == refresh_id)
        .unwrap();
    assert_eq!(refresh_row["status"], "pending");
    assert_eq!(
        refresh_row["payload"]["refresh"]["operation"],
        "project.external-refresh"
    );
    assert_eq!(refresh_row.get("commit"), None);

    let team_ack = rpc(
        &mut second_in,
        &mut second_out,
        10,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "electron-after-head-kill",
            "generation": 42,
            "batch_id": "selected-team-head",
            "operation": "commit-source",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(team_ack["result"]["ack"]["acknowledged"], true);
    let tail = pending(
        &mut second_in,
        &mut second_out,
        11,
        &root,
        "electron-after-head-kill",
        42,
    );
    assert_eq!(tail["result"]["envelopes"][0]["batch_id"], refresh_id);
    assert_eq!(tail["result"]["envelopes"][0]["status"], "pending");
    assert_eq!(file_snapshot(&remote), remote_after_commit);
    let tail_ack = rpc(
        &mut second_in,
        &mut second_out,
        12,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "electron-after-head-kill",
            "generation": 42,
            "batch_id": refresh_id,
            "operation": "project.external-refresh",
            "outcome": "coalesced",
        }),
    );
    assert_eq!(tail_ack["result"]["ack"]["acknowledged"], true);
    let drained = pending(
        &mut second_in,
        &mut second_out,
        13,
        &root,
        "electron-after-head-kill",
        42,
    );
    assert_eq!(drained["result"]["envelopes"], json!([]));

    let history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    let renderer_acknowledged = history
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| {
            row["batch_id"] == "selected-team-head"
                && row["status"] == "completed"
                && row["payload"]["phase"] == "renderer-acknowledged"
        })
        .count();
    assert_eq!(renderer_acknowledged, 1);
    assert_eq!(file_snapshot(&remote), remote_after_commit);

    let _ = second.kill();
    second.wait().unwrap();
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
    let reloaded = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
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

    let responsive = rpc(&mut stdin, &mut stdout, 4, "sys.version", json!({}));
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
    let reloaded = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
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
    let responsive = rpc(&mut stdin, &mut stdout, 12, "sys.version", json!({}));
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn project_configuration_workflow_persists_through_save_close_and_reopen() {
    struct Sidecar {
        child: std::process::Child,
        input: std::process::ChildStdin,
        output: BufReader<std::process::ChildStdout>,
    }

    fn spawn_sidecar(config: &std::path::Path) -> Sidecar {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("sidecar");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Sidecar {
            child,
            input,
            output,
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("configured-project");
    let remote = temp.path().join("configured-remote");
    std::fs::create_dir_all(&remote).unwrap();
    let mut first = spawn_sidecar(&config);
    rpc(
        &mut first.input,
        &mut first.output,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": true
        }),
    );

    let mut prefs = rpc(
        &mut first.input,
        &mut first.output,
        2,
        "prefs.get",
        json!({}),
    )["result"]
        .clone();
    prefs["filter_options"]["text"] = json!({"trim": "false"});
    prefs["srx_path"] = json!("");
    prefs["srx_xml"] = json!(
        r#"<?xml version="1.0"?><srx><body><languagerules><languagerule languagerulename="English"><rule break="no"><beforebreak>\.</beforebreak><afterbreak>\s</afterbreak></rule></languagerule></languagerules><maprules><maprule><languagemap languagepattern="en.*" languagerulename="English"/></maprule></maprules></body></srx>"#
    );
    let saved_prefs = rpc(&mut first.input, &mut first.output, 3, "prefs.set", prefs);
    assert_eq!(
        saved_prefs["result"]["filter_options"]["text"]["trim"],
        "false"
    );

    let source_dir = root.join("configured-source");
    let target_dir = root.join("configured-target");
    let tm_dir = root.join("configured-tm");
    let glossary_dir = root.join("configured-glossary");
    let glossary_file = glossary_dir.join("writeable.txt");
    let dictionary_dir = root.join("configured-dictionary");
    let export_tm_dir = root.join("configured-export");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("chapter.txt"), "One. Two").unwrap();
    let updated = rpc(
        &mut first.input,
        &mut first.output,
        4,
        "project.update",
        json!({
            "source_lang": "en",
            "target_lang": "de-DE",
            "source_tok": "org.omegat.tokenizer.DefaultTokenizer",
            "target_tok": "org.omegat.tokenizer.LuceneGermanTokenizer",
            "sentence_seg": true,
            "source_dir": source_dir,
            "target_dir": target_dir,
            "tm_dir": tm_dir,
            "glossary_dir": glossary_dir,
            "glossary_file": glossary_file,
            "dictionary_dir": dictionary_dir,
            "export_tm_dir": export_tm_dir,
            "export_tm_levels": "1,2",
            "support_default_translations": false,
            "remove_tags": true,
            "external_command": "printf configured",
            "source_dir_excludes": ["**/.ignored/**", "**/generated/**"],
            "transaction_project_root": root,
            "transaction_generation": 61,
            "transaction_batch_id": "configured-properties"
        }),
    );
    assert_eq!(updated["result"]["props"]["target_lang"], "de-DE");
    assert_eq!(
        updated["result"]["props"]["target_tok"],
        "org.omegat.tokenizer.LuceneGermanTokenizer"
    );
    assert_eq!(
        updated["result"]["props"]["source_dir_excludes"],
        json!(["**/.ignored/**", "**/generated/**"])
    );
    assert_eq!(
        updated["result"]["receipt"]["payload"]["operation"],
        "project.update"
    );
    let property_ack = rpc(
        &mut first.input,
        &mut first.output,
        5,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "configured-workflow",
            "generation": 61,
            "batch_id": "configured-properties",
            "operation": "project.update",
            "outcome": "succeeded"
        }),
    );
    assert_eq!(property_ack["result"]["ack"]["acknowledged"], true);
    let entries = rpc(
        &mut first.input,
        &mut first.output,
        6,
        "entry.list",
        json!({}),
    );
    assert_eq!(entries["result"].as_array().unwrap().len(), 1);
    assert_eq!(entries["result"][0]["source"], "One. Two");

    let mapping = rpc(
        &mut first.input,
        &mut first.output,
        7,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/source/",
                    "repository": "/source/",
                    "includes": ["**/*.txt"],
                    "excludes": ["**/.ignored/**"]
                }]
            }],
            "transaction_project_root": root,
            "transaction_generation": 61,
            "transaction_batch_id": "configured-mapping"
        }),
    );
    assert_eq!(mapping["result"]["props"]["has_repositories"], true);
    assert_eq!(
        mapping["result"]["receipt"]["payload"]["operation"],
        "team.mapping"
    );
    rpc(
        &mut first.input,
        &mut first.output,
        8,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "configured-workflow",
            "generation": 61,
            "batch_id": "configured-mapping",
            "operation": "team.mapping",
            "outcome": "succeeded"
        }),
    );

    let saved = rpc(
        &mut first.input,
        &mut first.output,
        9,
        "project.save",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 61,
            "transaction_batch_id": "configured-save"
        }),
    );
    assert_eq!(
        saved["result"]["receipt"]["payload"]["operation"],
        "project.save"
    );
    rpc(
        &mut first.input,
        &mut first.output,
        10,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "configured-workflow",
            "generation": 61,
            "batch_id": "configured-save",
            "operation": "project.save",
            "outcome": "succeeded"
        }),
    );
    let closed = rpc(
        &mut first.input,
        &mut first.output,
        11,
        "project.close",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 61,
            "transaction_batch_id": "configured-close"
        }),
    );
    assert_eq!(
        closed["result"]["receipt"]["payload"]["operation"],
        "project.close"
    );
    rpc(
        &mut first.input,
        &mut first.output,
        12,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "configured-workflow",
            "generation": 61,
            "batch_id": "configured-close",
            "operation": "project.close",
            "outcome": "succeeded"
        }),
    );
    first.child.kill().unwrap();
    first.child.wait().unwrap();

    let mut second = spawn_sidecar(&config);
    let reopened = rpc(
        &mut second.input,
        &mut second.output,
        13,
        "project.open",
        json!({"root": root}),
    );
    assert_eq!(reopened["result"]["target_lang"], "de-DE");
    assert_eq!(reopened["result"]["external_command"], "printf configured");
    assert_eq!(
        reopened["result"]["source_dir_excludes"],
        json!(["**/.ignored/**", "**/generated/**"])
    );
    assert_eq!(reopened["result"]["repositories"][0]["repo_type"], "file");
    assert_eq!(
        reopened["result"]["repositories"][0]["mappings"][0]["includes"],
        json!(["**/*.txt"])
    );
    let reopened_entries = rpc(
        &mut second.input,
        &mut second.output,
        14,
        "entry.list",
        json!({}),
    );
    assert_eq!(reopened_entries["result"].as_array().unwrap().len(), 1);
    assert_eq!(reopened_entries["result"][0]["source"], "One. Two");
    let reopened_prefs = rpc(
        &mut second.input,
        &mut second.output,
        15,
        "prefs.get",
        json!({}),
    );
    assert_eq!(
        reopened_prefs["result"]["filter_options"]["text"]["trim"],
        "false"
    );
    assert!(reopened_prefs["result"]["srx_xml"]
        .as_str()
        .unwrap()
        .contains("languagemap"));
    let _ = second.child.kill();
}

#[test]
fn protocol_cancellation_rolls_back_reload_and_compile_state() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("cancel-reload-compile");
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

    for index in 0..1_000 {
        std::fs::write(
            root.join("source").join(format!("{index:04}.txt")),
            "Repeated source",
        )
        .unwrap();
    }
    let loaded = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
    assert_eq!(loaded["result"]["entries"], 1_000);
    let before_reload = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}))["result"].clone();

    for index in 0..1_000 {
        std::fs::write(
            root.join("source").join(format!("{index:04}.txt")),
            format!("Changed source {index}"),
        )
        .unwrap();
    }
    let cancelled_reload = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        4,
        "project.reload",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 9,
            "transaction_batch_id": "cancelled-scoped-reload"
        }),
        "project.reload.sources",
    );
    assert_eq!(
        cancelled_reload["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    let after_reload = rpc(&mut stdin, &mut stdout, 5, "entry.list", json!({}));
    assert_eq!(after_reload["result"], before_reload);

    let cancelled_external_refresh = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        50,
        "project.external-refresh",
        json!({}),
        "project.external-refresh.sources",
    );
    assert_eq!(
        cancelled_external_refresh["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    let after_external_refresh = rpc(&mut stdin, &mut stdout, 51, "entry.list", json!({}));
    assert_eq!(after_external_refresh["result"], before_reload);

    let first = &after_reload["result"][0];
    let updated = rpc(
        &mut stdin,
        &mut stdout,
        6,
        "entry.set",
        json!({
            "index": 0,
            "key": first["key"],
            "translation": "Traduction partagée",
            "note": "",
            "revision": first["revision"],
            "default_translation": true
        }),
    );
    assert_eq!(
        updated["result"]["updated"].as_array().unwrap().len(),
        1_000
    );
    std::fs::write(root.join("target/0000.txt"), "old compiled target").unwrap();
    std::fs::write(root.join("target/unrelated.keep"), "must remain").unwrap();
    let before_compile = file_snapshot(&root.join("target"));
    let cancelled_compile = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        7,
        "project.compile",
        json!({
            "transaction_project_root": root,
            "transaction_generation": 9,
            "transaction_batch_id": "cancelled-scoped-compile"
        }),
        "project.compile.targets",
    );
    assert_eq!(
        cancelled_compile["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(file_snapshot(&root.join("target")), before_compile);
    assert!(std::fs::read_dir(root.join("target"))
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".omegat-compile-")));
    assert!(!root.join(".repositories/transactions/active.json").exists());

    let responsive = rpc(&mut stdin, &mut stdout, 8, "sys.version", json!({}));
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn protocol_cancellation_rolls_back_team_sync_and_commit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("cancel-team");
    let remote = temp.path().join("file-remote");
    std::fs::create_dir_all(remote.join("source")).unwrap();
    for index in 0..600 {
        std::fs::write(
            remote.join("source").join(format!("{index:04}.txt")),
            format!("remote baseline {index:04} {}", "x".repeat(256)),
        )
        .unwrap();
    }
    let _ = rpc(
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
    let mapping = json!({
        "repositories": [{
            "repo_type": "file",
            "url": remote,
            "branch": null,
            "mappings": [{
                "local": "/source/",
                "repository": "/source/",
                "includes": [],
                "excludes": []
            }]
        }]
    });
    let configured = rpc(&mut stdin, &mut stdout, 2, "team.mapping", mapping);
    assert_eq!(configured["result"]["ok"], true);
    let initialized = rpc(&mut stdin, &mut stdout, 3, "team.sync", json!({}));
    assert_eq!(initialized["result"]["action"], "sync");

    for index in 0..600 {
        std::fs::write(
            root.join("source").join(format!("{index:04}.txt")),
            format!("local commit candidate {index:04} {}", "y".repeat(256)),
        )
        .unwrap();
    }
    let project_before_commit = file_snapshot(&root.join("source"));
    let remote_before_commit = file_snapshot(&remote);
    let cancelled_commit = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        4,
        "team.commit",
        json!({"which": "source"}),
        "team.mapping.copy",
    );
    assert_eq!(
        cancelled_commit["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(file_snapshot(&root.join("source")), project_before_commit);
    assert_eq!(file_snapshot(&remote), remote_before_commit);
    assert!(!root.join(".repositories/transactions/active.json").exists());

    for index in 0..600 {
        std::fs::write(
            remote.join("source").join(format!("{index:04}.txt")),
            format!("remote sync candidate {index:04} {}", "z".repeat(256)),
        )
        .unwrap();
    }
    let project_before_sync = file_snapshot(&root.join("source"));
    let remote_before_sync = file_snapshot(&remote);
    let cancelled_sync = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        5,
        "team.sync",
        json!({}),
        "team.mapping.copy",
    );
    assert_eq!(
        cancelled_sync["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(file_snapshot(&root.join("source")), project_before_sync);
    assert_eq!(file_snapshot(&remote), remote_before_sync);
    assert!(!root.join(".repositories/transactions/active.json").exists());

    let responsive = rpc(&mut stdin, &mut stdout, 6, "sys.version", json!({}));
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn protocol_cancellation_rolls_back_team_conflict_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let retry_wait_marker = temp.path().join("cancel-owner-wait.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .env(
            "OMEGAT_TEST_TRANSACTION_OWNER_RETRY_WAIT_MARKER",
            &retry_wait_marker,
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let root = temp.path().join("cancel-team-resolve");
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

    let mut tmx =
        String::from(r#"<?xml version="1.0" encoding="UTF-8"?><tmx version="1.4"><body>"#);
    tmx.push_str(
        r#"<tu><tuv xml:lang="en"><seg>cancel me</seg></tuv><tuv xml:lang="fr"><seg>ours</seg></tuv></tu>"#,
    );
    tmx.push_str("</body></tmx>");
    let save_tmx = root.join("omegat/project_save.tmx");
    std::fs::write(&save_tmx, tmx).unwrap();
    for index in 0..2_000 {
        std::fs::write(
            root.join("source").join(format!("{index:04}.txt")),
            format!("snapshot cancellation source {index}"),
        )
        .unwrap();
    }
    let prep = root.join(".repositories/prep");
    std::fs::create_dir_all(&prep).unwrap();
    let conflicts = vec![
        json!({
            "kind": "tmx",
            "source": "cancel me",
            "ours": "ours",
            "theirs": "theirs",
            "message": "TMX conflict on cancel me"
        }),
        json!({
            "kind": "glossary",
            "source": "other conflict",
            "ours": "other ours",
            "theirs": "other theirs",
            "message": "glossary conflict on other conflict"
        }),
    ];
    let conflicts_path = prep.join("conflicts.json");
    std::fs::write(
        &conflicts_path,
        serde_json::to_vec_pretty(&conflicts).unwrap(),
    )
    .unwrap();
    let tmx_before = std::fs::read(&save_tmx).unwrap();
    let conflicts_before = std::fs::read(&conflicts_path).unwrap();

    let cancelled = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        2,
        "team.resolve",
        json!({
            "source": "cancel me",
            "side": "theirs",
            "transaction_project_root": root,
            "transaction_generation": 44,
            "transaction_batch_id": "conflict-envelope-44"
        }),
        "team.resolve.snapshot",
    );
    assert_eq!(
        cancelled["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before);
    assert_eq!(std::fs::read(&conflicts_path).unwrap(), conflicts_before);
    assert!(!prep.join("resolved.json").exists());
    assert!(!root.join(".repositories/transactions/active.json").exists());
    assert!(std::fs::read_dir(root.join(".repositories/transactions"))
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".snapshot")));

    let team_history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    let team_envelope: omegat_team::TransactionEnvelope<Value> =
        serde_json::from_str(team_history.lines().last().unwrap()).unwrap();
    assert_eq!(team_envelope.version, 1);
    assert_eq!(team_envelope.project_root, root.canonicalize().unwrap());
    assert_eq!(team_envelope.generation, 44);
    assert_eq!(team_envelope.batch_id, "conflict-envelope-44");
    assert_eq!(
        team_envelope.status,
        omegat_team::TransactionStatus::RequestCancelled
    );
    assert_eq!(team_envelope.error_code, Some(-32800));

    let dispatch_cancel_batch = "conflict-dispatch-cancel-45";
    let committed = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "team.resolve",
        json!({
            "source": "cancel me",
            "side": "ours",
            "transaction_project_root": root,
            "transaction_generation": 45,
            "transaction_batch_id": dispatch_cancel_batch,
        }),
    );
    assert_eq!(
        committed["result"]["receipt"]["batch_id"],
        dispatch_cancel_batch
    );
    assert_eq!(
        committed["result"]["receipt"]["status"],
        "sidecar_committed"
    );
    let selected = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "resolve-dispatch-cancel-owner",
            "generation": 45,
        }),
    );
    assert_eq!(
        selected["result"]["envelopes"][0]["batch_id"],
        dispatch_cancel_batch
    );
    let owner_before_cancel: Value = serde_json::from_slice(
        &std::fs::read(root.join(".repositories/transactions/renderer-owner.json")).unwrap(),
    )
    .unwrap();
    let cancelled_owner_wait = send_cancelled_request(
        &mut stdin,
        &mut stdout,
        5,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "resolve-dispatch-cancel-contender",
            "generation": 46,
            "owner_retry_timeout_ms": 20_000,
            "owner_retry_attempts": 2,
        }),
        || {
            for _ in 0..1_000 {
                if retry_wait_marker.is_file() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("resolve contender did not enter the owner-election wait");
        },
    );
    assert_eq!(
        cancelled_owner_wait["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &std::fs::read(root.join(".repositories/transactions/renderer-owner.json")).unwrap()
        )
        .unwrap(),
        owner_before_cancel,
        "cancelled contender replaced the live dispatcher owner"
    );

    let cancelled_after_claim = rpc(
        &mut stdin,
        &mut stdout,
        6,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "resolve-dispatch-cancel-owner",
            "generation": 45,
            "batch_id": dispatch_cancel_batch,
            "operation": "resolve-conflict",
            "outcome": "cancelled",
        }),
    );
    assert_eq!(
        cancelled_after_claim["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before);
    assert_eq!(std::fs::read(&conflicts_path).unwrap(), conflicts_before);
    assert!(!prep.join("resolved.json").exists());
    assert!(!root.join(".repositories/transactions/active.json").exists());
    let owner_after_cancel: Value = serde_json::from_slice(
        &std::fs::read(root.join(".repositories/transactions/renderer-owner.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(owner_after_cancel, owner_before_cancel);
    let cancelled_history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    let cancelled_dispatch_rows = cancelled_history
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| {
            row["batch_id"] == dispatch_cancel_batch
                && row["status"] == "request_cancelled"
                && row["error_code"] == -32800
                && row["payload"]["phase"] == "renderer-cancelled"
        })
        .count();
    assert_eq!(cancelled_dispatch_rows, 1);
    let no_cancelled_delivery = rpc(
        &mut stdin,
        &mut stdout,
        7,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "resolve-dispatch-cancel-owner",
            "generation": 45,
        }),
    );
    assert_eq!(no_cancelled_delivery["result"]["envelopes"], json!([]));

    let refresh = rpc(
        &mut stdin,
        &mut stdout,
        8,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "unified-envelope-contract",
            "generation": 44,
            "paths": [root.join("source/0000.txt")],
            "fingerprints": { "source/0000.txt": "after-conflict-cancel" },
            "sources": ["native"]
        }),
    );
    let refresh_envelope: omegat_team::TransactionEnvelope<Value> =
        serde_json::from_value(refresh["result"]["batch"].clone()).unwrap();
    let shared_journal: Value = serde_json::from_slice(
        &std::fs::read(root.join(".repositories/transactions/active.json")).unwrap(),
    )
    .unwrap();
    let persisted_refresh = &shared_journal["batches"][0];
    assert_eq!(persisted_refresh["version"], refresh_envelope.version);
    assert_eq!(
        persisted_refresh["project_root"],
        json!(refresh_envelope.project_root)
    );
    assert_eq!(persisted_refresh["generation"], refresh_envelope.generation);
    assert_eq!(persisted_refresh["batch_id"], refresh_envelope.batch_id);
    assert_eq!(persisted_refresh["status"], "pending");
    assert_eq!(
        persisted_refresh["payload"]["refresh"],
        refresh_envelope.payload
    );
    assert_eq!(refresh_envelope.version, team_envelope.version);
    assert_eq!(refresh_envelope.project_root, team_envelope.project_root);
    assert_eq!(refresh_envelope.generation, team_envelope.generation);
    assert_eq!(
        refresh_envelope.status,
        omegat_team::TransactionStatus::Pending
    );
    assert_eq!(refresh_envelope.error_code, None);
    assert!(refresh_envelope.batch_id.starts_with("refresh-"));

    let responsive = rpc(&mut stdin, &mut stdout, 9, "sys.version", json!({}));
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn resolve_receipt_can_be_cancelled_at_the_global_fifo_tail() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("resolve-fifo-tail");
    let remote = temp.path().join("resolve-fifo-remote");
    std::fs::create_dir_all(remote.join("target")).unwrap();
    std::fs::write(remote.join("target/team.txt"), "remote-before").unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    rpc(
        &mut stdin,
        &mut stdout,
        2,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [{
                    "local": "/target/team.txt",
                    "repository": "/target/team.txt",
                    "includes": [],
                    "excludes": [],
                }],
            }],
        }),
    );
    rpc(&mut stdin, &mut stdout, 3, "team.sync", json!({}));
    std::fs::write(root.join("source/source.txt"), "fifo conflict").unwrap();
    rpc(&mut stdin, &mut stdout, 4, "project.reload", json!({}));

    let generation = 71;
    let heads = [
        ("fifo-save-head", "project.save"),
        ("fifo-close-head", "project.close"),
        ("fifo-sync-head", "sync"),
    ];
    let saved = rpc(
        &mut stdin,
        &mut stdout,
        5,
        "project.save",
        json!({
            "transaction_project_root": root,
            "transaction_generation": generation,
            "transaction_batch_id": heads[0].0,
        }),
    );
    assert_eq!(
        saved["result"]["receipt"]["payload"]["operation"],
        heads[0].1
    );
    let closed = rpc(
        &mut stdin,
        &mut stdout,
        6,
        "project.close",
        json!({
            "transaction_project_root": root,
            "transaction_generation": generation,
            "transaction_batch_id": heads[1].0,
        }),
    );
    assert_eq!(
        closed["result"]["receipt"]["payload"]["operation"],
        heads[1].1
    );
    rpc(
        &mut stdin,
        &mut stdout,
        7,
        "project.open",
        json!({ "root": root }),
    );
    let synced = rpc(
        &mut stdin,
        &mut stdout,
        8,
        "team.sync",
        json!({
            "transaction_project_root": root,
            "transaction_generation": generation,
            "transaction_batch_id": heads[2].0,
        }),
    );
    assert_eq!(
        synced["result"]["receipt"]["payload"]["operation"],
        heads[2].1
    );

    let save_tmx = root.join("omegat/project_save.tmx");
    let ours_tmx = r#"<?xml version="1.0" encoding="UTF-8"?><tmx version="1.4"><body><tu><tuv xml:lang="en"><seg>fifo conflict</seg></tuv><tuv xml:lang="fr"><seg>ours</seg></tuv></tu></body></tmx>"#;
    std::fs::write(&save_tmx, ours_tmx).unwrap();
    let prep = root.join(".repositories/prep");
    std::fs::create_dir_all(&prep).unwrap();
    let conflicts_path = prep.join("conflicts.json");
    std::fs::write(
        &conflicts_path,
        serde_json::to_vec_pretty(&json!([{
            "kind": "tmx",
            "source": "fifo conflict",
            "ours": "ours",
            "theirs": "theirs",
            "message": "TMX conflict on fifo conflict",
        }]))
        .unwrap(),
    )
    .unwrap();
    rpc(&mut stdin, &mut stdout, 9, "project.reload", json!({}));
    let tmx_before_resolve = std::fs::read(&save_tmx).unwrap();
    let conflicts_before_resolve = std::fs::read(&conflicts_path).unwrap();
    let resolve_batch = "fifo-resolve-tail";
    let resolved = rpc(
        &mut stdin,
        &mut stdout,
        10,
        "team.resolve",
        json!({
            "source": "fifo conflict",
            "side": "theirs",
            "transaction_project_root": root,
            "transaction_generation": generation,
            "transaction_batch_id": resolve_batch,
        }),
    );
    assert_eq!(
        resolved["result"]["receipt"]["payload"]["operation"],
        "resolve-conflict"
    );

    let selected = rpc(
        &mut stdin,
        &mut stdout,
        11,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "fifo-owner",
            "generation": generation,
        }),
    );
    assert_eq!(selected["result"]["envelopes"][0]["batch_id"], heads[0].0);
    let owner_before_cancel =
        std::fs::read(root.join(".repositories/transactions/renderer-owner.json")).unwrap();
    let cancelled = rpc(
        &mut stdin,
        &mut stdout,
        12,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "fifo-owner",
            "generation": generation,
            "batch_id": resolve_batch,
            "operation": "resolve-conflict",
            "outcome": "cancelled",
        }),
    );
    assert_eq!(
        cancelled["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before_resolve);
    assert_eq!(
        std::fs::read(&conflicts_path).unwrap(),
        conflicts_before_resolve
    );
    assert_eq!(
        std::fs::read(root.join(".repositories/transactions/renderer-owner.json")).unwrap(),
        owner_before_cancel
    );
    let queue: Value = serde_json::from_slice(
        &std::fs::read(root.join(".repositories/transactions/active.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        queue["batches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["batch_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        heads.iter().map(|head| head.0).collect::<Vec<_>>()
    );

    for (offset, (batch_id, operation)) in heads.into_iter().enumerate() {
        let pending = rpc(
            &mut stdin,
            &mut stdout,
            20 + offset as i64 * 2,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": "fifo-owner",
                "generation": generation,
            }),
        );
        assert_eq!(pending["result"]["envelopes"][0]["batch_id"], batch_id);
        assert_ne!(
            pending["result"]["envelopes"][0]["batch_id"], resolve_batch,
            "cancelled resolve tail was selected for delivery"
        );
        let ack = rpc(
            &mut stdin,
            &mut stdout,
            21 + offset as i64 * 2,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": "fifo-owner",
                "generation": generation,
                "batch_id": batch_id,
                "operation": operation,
                "outcome": "succeeded",
            }),
        );
        assert_eq!(ack["result"]["ack"]["acknowledged"], true);
    }
    let drained = rpc(
        &mut stdin,
        &mut stdout,
        30,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "fifo-owner",
            "generation": generation,
        }),
    );
    assert_eq!(drained["result"]["envelopes"], json!([]));
    let history =
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson")).unwrap();
    assert_eq!(
        history
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| {
                row["batch_id"] == resolve_batch
                    && row["status"] == "request_cancelled"
                    && row["error_code"] == -32800
            })
            .count(),
        1
    );
    let _ = child.kill();
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_cancellation_recovery_wins_owner_death_at_each_durable_boundary() {
    fn spawn_sidecar(
        config: &std::path::Path,
        checkpoint: Option<(&str, &std::path::Path)>,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        if let Some((point, marker)) = checkpoint {
            command
                .env("OMEGAT_TEST_RESOLVE_CANCELLATION_POINT", point)
                .env("OMEGAT_TEST_RESOLVE_CANCELLATION_MARKER", marker);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        (child, input, output)
    }

    for point in [
        "after_intent_queue_rename",
        "after_rollback_fsync",
        "after_terminal_queue_rename",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join(format!("cancel-owner-death-{point}"));
        let marker = temp.path().join(format!("{point}.marker"));
        let save_tmx = root.join("omegat/project_save.tmx");
        let prep = root.join(".repositories/prep");
        let conflicts_path = prep.join("conflicts.json");
        let active_path = root.join(".repositories/transactions/active.json");
        let owner_path = root.join(".repositories/transactions/renderer-owner.json");
        let history_path = root.join(".repositories/transactions/history.ndjson");
        let batch_id = format!("cancel-owner-death-{point}");
        let generation = 81;

        let (mut interrupted, mut interrupted_in, mut interrupted_out) =
            spawn_sidecar(&config, Some((point, &marker)));
        rpc(
            &mut interrupted_in,
            &mut interrupted_out,
            1,
            "project.create",
            json!({
                "root": root,
                "source_lang": "en",
                "target_lang": "fr",
                "sentence_seg": false,
            }),
        );
        std::fs::write(root.join("source/source.txt"), "owner death conflict").unwrap();
        let ours_tmx = r#"<?xml version="1.0" encoding="UTF-8"?><tmx version="1.4"><body><tu><tuv xml:lang="en"><seg>owner death conflict</seg></tuv><tuv xml:lang="fr"><seg>ours</seg></tuv></tu></body></tmx>"#;
        std::fs::write(&save_tmx, ours_tmx).unwrap();
        std::fs::create_dir_all(&prep).unwrap();
        let conflicts = serde_json::to_vec_pretty(&json!([{
            "kind": "tmx",
            "source": "owner death conflict",
            "ours": "ours",
            "theirs": "theirs",
            "message": "TMX conflict on owner death conflict",
        }]))
        .unwrap();
        std::fs::write(&conflicts_path, &conflicts).unwrap();
        rpc(
            &mut interrupted_in,
            &mut interrupted_out,
            2,
            "project.reload",
            json!({}),
        );
        let tmx_before = std::fs::read(&save_tmx).unwrap();
        let conflicts_before = std::fs::read(&conflicts_path).unwrap();
        let committed = rpc(
            &mut interrupted_in,
            &mut interrupted_out,
            3,
            "team.resolve",
            json!({
                "source": "owner death conflict",
                "side": "theirs",
                "transaction_project_root": root,
                "transaction_generation": generation,
                "transaction_batch_id": batch_id,
            }),
        );
        assert_eq!(
            committed["result"]["receipt"]["payload"]["operation"],
            "resolve-conflict"
        );
        assert_ne!(std::fs::read(&save_tmx).unwrap(), tmx_before);

        let mut durable_owner = Command::new("sleep").arg("60").spawn().unwrap();
        let owner_pid = durable_owner.id();
        let selected = rpc(
            &mut interrupted_in,
            &mut interrupted_out,
            4,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": format!("old-owner-{point}"),
                "owner_process_id": owner_pid,
                "generation": generation,
            }),
        );
        assert_eq!(selected["result"]["envelopes"][0]["batch_id"], batch_id);
        let old_claim: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(old_claim["process_id"], owner_pid);

        writeln!(
            interrupted_in,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "transaction.receipt.ack",
                "params": {
                    "root": root,
                    "app_instance": format!("old-owner-{point}"),
                    "owner_process_id": owner_pid,
                    "generation": generation,
                    "batch_id": batch_id,
                    "operation": "resolve-conflict",
                    "outcome": "cancelled",
                },
            })
        )
        .unwrap();
        interrupted_in.flush().unwrap();
        for _ in 0..1_000 {
            assert!(
                interrupted.try_wait().unwrap().is_none(),
                "cancelling sidecar exited before {point}"
            );
            if marker.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.is_file(), "cancellation did not reach {point}");
        let parked: Value = serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
        let parked_row = parked["batches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["batch_id"] == batch_id)
            .unwrap();
        assert_eq!(
            parked_row["status"],
            if point == "after_terminal_queue_rename" {
                "request_cancelled"
            } else {
                "cancellation_pending"
            }
        );
        if point == "after_intent_queue_rename" {
            assert_ne!(std::fs::read(&save_tmx).unwrap(), tmx_before);
        } else {
            assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before);
            assert_eq!(std::fs::read(&conflicts_path).unwrap(), conflicts_before);
        }

        // The user cancellation has crossed its atomic intent rename. Kill the
        // durable Electron owner and its cancelling sidecar in the same window.
        durable_owner.kill().unwrap();
        interrupted.kill().unwrap();
        durable_owner.wait().unwrap();
        interrupted.wait().unwrap();
        assert!(!std::path::Path::new("/proc")
            .join(owner_pid.to_string())
            .exists());

        let (mut replacement, mut replacement_in, mut replacement_out) =
            spawn_sidecar(&config, None);
        if point != "after_terminal_queue_rename" {
            // This is a second user cancellation, not project-open recovery.
            // It takes over the exact durable intent left by the dead first
            // caller and must not create another rollback owner or terminal.
            let second_cancel = rpc(
                &mut replacement_in,
                &mut replacement_out,
                6,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": format!("second-cancel-{point}"),
                    "owner_process_id": replacement.id(),
                    "generation": generation,
                    "batch_id": batch_id,
                    "operation": "resolve-conflict",
                    "outcome": "cancelled",
                }),
            );
            assert_eq!(
                second_cancel["error"],
                json!({"code": -32800, "message": "request cancelled"})
            );
        }
        let opened = rpc(
            &mut replacement_in,
            &mut replacement_out,
            7,
            "project.open",
            json!({ "root": root }),
        );
        assert_eq!(opened["result"]["root"], root.to_string_lossy().as_ref());
        assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before);
        assert_eq!(std::fs::read(&conflicts_path).unwrap(), conflicts_before);
        assert!(!active_path.exists());

        let no_resolve_winner = rpc(
            &mut replacement_in,
            &mut replacement_out,
            8,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": format!("replacement-{point}"),
                "owner_process_id": replacement.id(),
                "generation": generation + 1,
            }),
        );
        assert_eq!(no_resolve_winner["result"]["envelopes"], json!([]));
        let new_claim: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(new_claim["process_id"], replacement.id());
        assert_ne!(new_claim["claim_id"], old_claim["claim_id"]);

        let cancellation_ack = rpc(
            &mut replacement_in,
            &mut replacement_out,
            9,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": format!("replacement-{point}"),
                "owner_process_id": replacement.id(),
                "generation": generation,
                "batch_id": batch_id,
                "operation": "resolve-conflict",
                "outcome": "cancelled",
            }),
        );
        assert_eq!(
            cancellation_ack["error"],
            json!({"code": -32800, "message": "request cancelled"})
        );
        let history = std::fs::read_to_string(&history_path).unwrap();
        assert_eq!(
            history
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| {
                    row["batch_id"] == batch_id
                        && row["status"] == "request_cancelled"
                        && row["error_code"] == -32800
                })
                .count(),
            1,
            "cancellation recovery duplicated its terminal row at {point}"
        );
        assert_eq!(
            history
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| {
                    row["batch_id"] == batch_id
                        && row["status"] == "cancellation_pending"
                        && row["payload"]["phase"] == "renderer-rollback-durable"
                })
                .count(),
            1,
            "second cancellation opened another durable rollback at {point}"
        );
        if point != "after_terminal_queue_rename" {
            assert_eq!(
                history
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap())
                    .filter(|row| {
                        row["batch_id"] == batch_id
                            && row["status"] == "request_cancelled"
                            && row["payload"]["phase"] == "renderer-cancelled-takeover"
                    })
                    .count(),
                1,
                "second cancellation did not own the sole terminal at {point}"
            );
        }
        let responsive = rpc(
            &mut replacement_in,
            &mut replacement_out,
            10,
            "sys.version",
            json!({}),
        );
        assert_eq!(responsive["result"]["version"], "6.2.0");
        replacement.kill().unwrap();
        replacement.wait().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn waiting_raw_cancel_callers_survive_rollback_and_terminal_publisher_deaths() {
    fn spawn_sidecar(
        config: &std::path::Path,
        checkpoint: Option<(&str, &std::path::Path)>,
        followup_checkpoint: Option<(&str, &std::path::Path)>,
        wait_marker: Option<&std::path::Path>,
        takeover_marker: Option<&std::path::Path>,
        compaction_checkpoint: Option<(&str, &std::path::Path, &std::path::Path)>,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        if let Some((point, owner_marker)) = checkpoint {
            command
                .env("OMEGAT_TEST_RESOLVE_CANCELLATION_POINT", point)
                .env("OMEGAT_TEST_RESOLVE_CANCELLATION_MARKER", owner_marker);
        }
        if let Some((point, owner_marker)) = followup_checkpoint {
            command
                .env("OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_POINT", point)
                .env(
                    "OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_MARKER",
                    owner_marker,
                );
        }
        if let Some(wait_marker) = wait_marker {
            command.env("OMEGAT_TEST_RESOLVE_CANCELLATION_WAIT_MARKER", wait_marker);
        }
        if let Some(takeover_marker) = takeover_marker {
            command.env(
                "OMEGAT_TEST_RESOLVE_CANCELLATION_TAKEOVER_MARKER",
                takeover_marker,
            );
        }
        if let Some((point, marker, release)) = compaction_checkpoint {
            command
                .env("OMEGAT_TEST_PRODUCT_COMPACTION_POINT", point)
                .env("OMEGAT_TEST_PRODUCT_COMPACTION_MARKER", marker)
                .env("OMEGAT_TEST_PRODUCT_COMPACTION_RELEASE", release);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        (child, input, output)
    }

    fn wait_for_file(path: &std::path::Path, process: &mut std::process::Child) {
        for _ in 0..1_000 {
            assert!(
                process.try_wait().unwrap().is_none(),
                "sidecar exited before marker {}",
                path.display()
            );
            if path.is_file() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {}", path.display());
    }

    fn start_cancel_call(
        mut input: std::process::ChildStdin,
        mut output: BufReader<std::process::ChildStdout>,
        id: i64,
        root: std::path::PathBuf,
        app_instance: String,
        owner_process_id: u32,
        generation: u64,
        batch_id: String,
    ) -> std::thread::JoinHandle<(
        Option<Value>,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    )> {
        std::thread::spawn(move || {
            writeln!(
                input,
                "{}",
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "transaction.receipt.ack",
                    "params": {
                        "root": root,
                        "app_instance": app_instance,
                        "owner_process_id": owner_process_id,
                        "generation": generation,
                        "batch_id": batch_id,
                        "operation": "resolve-conflict",
                        "outcome": "cancelled",
                    },
                })
            )
            .unwrap();
            input.flush().unwrap();
            let response = loop {
                let mut line = String::new();
                match output.read_line(&mut line) {
                    Ok(0) | Err(_) => break None,
                    Ok(_) => {
                        let Ok(value) = serde_json::from_str::<Value>(&line) else {
                            break None;
                        };
                        if value.get("id").and_then(Value::as_i64) == Some(id) {
                            break Some(value);
                        }
                    }
                }
            };
            (response, input, output)
        })
    }

    for (point, compaction_point) in [
        ("after_intent_queue_rename", None),
        ("after_intent_queue_rename", Some("after_archive_fsync")),
        ("after_intent_queue_rename", Some("after_queue_rename")),
        ("after_rollback_fsync", None),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join("raw-cancel-owner-takeover");
        let remote = temp.path().join("raw-cancel-owner-remote");
        let owner_marker = temp.path().join("cancel-owner.json");
        let rollback_owner_marker = temp.path().join("cancel-rollback-owner.json");
        let terminal_owner_marker = temp.path().join("cancel-terminal-owner.json");
        let compaction_marker = temp.path().join("cancel-compaction.marker");
        let compaction_release = temp.path().join("cancel-compaction.release");
        let save_tmx = root.join("omegat/project_save.tmx");
        let prep = root.join(".repositories/prep");
        let conflicts_path = prep.join("conflicts.json");
        let active_path = root.join(".repositories/transactions/active.json");
        let history_path = root.join(".repositories/transactions/history.ndjson");
        let batch_id = "raw-cancel-owner-takeover";
        let generation = 93;
        let fifo_heads = [
            ("raw-cancel-fifo-sync", "sync"),
            ("raw-cancel-fifo-save", "project.save"),
            ("raw-cancel-fifo-close", "project.close"),
        ];

        let (mut owner, mut owner_in, mut owner_out) = spawn_sidecar(
            &config,
            Some((point, &owner_marker)),
            None,
            None,
            None,
            None,
        );
        rpc(
            &mut owner_in,
            &mut owner_out,
            1,
            "project.create",
            json!({
                "root": root,
                "source_lang": "en",
                "target_lang": "fr",
                "sentence_seg": false,
            }),
        );
        std::fs::create_dir_all(remote.join("target")).unwrap();
        std::fs::write(remote.join("target/team.txt"), "raw owner remote").unwrap();
        rpc(
            &mut owner_in,
            &mut owner_out,
            2,
            "team.mapping",
            json!({
                "repositories": [{
                    "repo_type": "file",
                    "url": remote,
                    "branch": null,
                    "mappings": [{
                        "local": "/target/team.txt",
                        "repository": "/target/team.txt",
                        "includes": [],
                        "excludes": [],
                    }],
                }],
            }),
        );
        let synced = rpc(
            &mut owner_in,
            &mut owner_out,
            3,
            "team.sync",
            json!({
                "transaction_project_root": root,
                "transaction_generation": generation,
                "transaction_batch_id": fifo_heads[0].0,
            }),
        );
        assert_eq!(
            synced["result"]["receipt"]["payload"]["operation"],
            fifo_heads[0].1
        );
        let saved = rpc(
            &mut owner_in,
            &mut owner_out,
            4,
            "project.save",
            json!({
                "transaction_project_root": root,
                "transaction_generation": generation,
                "transaction_batch_id": fifo_heads[1].0,
            }),
        );
        assert_eq!(
            saved["result"]["receipt"]["payload"]["operation"],
            fifo_heads[1].1
        );
        let closed = rpc(
            &mut owner_in,
            &mut owner_out,
            5,
            "project.close",
            json!({
                "transaction_project_root": root,
                "transaction_generation": generation,
                "transaction_batch_id": fifo_heads[2].0,
            }),
        );
        assert_eq!(
            closed["result"]["receipt"]["payload"]["operation"],
            fifo_heads[2].1
        );
        rpc(
            &mut owner_in,
            &mut owner_out,
            6,
            "project.open",
            json!({ "root": root }),
        );
        std::fs::write(root.join("source/wanted.txt"), "raw owner conflict").unwrap();
        std::fs::write(root.join("source/decoy.txt"), "raw owner decoy").unwrap();
        let original_tmx = r#"<?xml version="1.0" encoding="UTF-8"?><tmx version="1.4"><body><tu><tuv xml:lang="en"><seg>raw owner conflict</seg></tuv><tuv xml:lang="fr"><seg>ours</seg></tuv></tu></body></tmx>"#;
        std::fs::write(&save_tmx, original_tmx).unwrap();
        std::fs::create_dir_all(&prep).unwrap();
        let original_conflicts = serde_json::to_vec_pretty(&json!([{
            "kind": "tmx",
            "source": "raw owner conflict",
            "ours": "ours",
            "theirs": "theirs",
            "message": "raw owner conflict",
        }]))
        .unwrap();
        std::fs::write(&conflicts_path, &original_conflicts).unwrap();
        rpc(
            &mut owner_in,
            &mut owner_out,
            7,
            "project.reload",
            json!({}),
        );
        let committed = rpc(
            &mut owner_in,
            &mut owner_out,
            8,
            "team.resolve",
            json!({
                "source": "raw owner conflict",
                "side": "theirs",
                "transaction_project_root": root,
                "transaction_generation": generation,
                "transaction_batch_id": batch_id,
            }),
        );
        assert_eq!(
            committed["result"]["receipt"]["payload"]["operation"],
            "resolve-conflict"
        );
        assert_ne!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());

        writeln!(
            owner_in,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "transaction.receipt.ack",
                "params": {
                    "root": root,
                    "app_instance": "raw-cancel-owner",
                    "owner_process_id": owner.id(),
                    "generation": generation,
                    "batch_id": batch_id,
                    "operation": "resolve-conflict",
                    "outcome": "cancelled",
                },
            })
        )
        .unwrap();
        owner_in.flush().unwrap();
        wait_for_file(&owner_marker, &mut owner);
        if point == "after_intent_queue_rename" {
            assert_ne!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());
            assert_ne!(std::fs::read(&conflicts_path).unwrap(), original_conflicts);
        } else {
            assert_eq!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());
            assert_eq!(std::fs::read(&conflicts_path).unwrap(), original_conflicts);
        }

        let waiter_count = if point == "after_intent_queue_rename" {
            3
        } else {
            1
        };
        let mut waiters = Vec::new();
        for index in 0..waiter_count {
            let wait_marker = temp.path().join(format!("cancel-waiter-{index}.json"));
            let takeover_marker = temp.path().join(format!("cancel-takeover-{index}.json"));
            let checkpoint = (point == "after_intent_queue_rename")
                .then_some(("after_rollback_fsync", rollback_owner_marker.as_path()));
            let followup_checkpoint = (point == "after_intent_queue_rename").then_some((
                "after_terminal_queue_rename",
                terminal_owner_marker.as_path(),
            ));
            let (child, input, output) = spawn_sidecar(
                &config,
                checkpoint,
                followup_checkpoint,
                Some(&wait_marker),
                Some(&takeover_marker),
                compaction_point.map(|compaction_point| {
                    (
                        compaction_point,
                        compaction_marker.as_path(),
                        compaction_release.as_path(),
                    )
                }),
            );
            let pid = child.id();
            let call = start_cancel_call(
                input,
                output,
                5 + index as i64,
                root.clone(),
                format!("raw-cancel-waiter-{index}"),
                pid,
                generation,
                batch_id.to_string(),
            );
            waiters.push((child, pid, Some(call), wait_marker, takeover_marker));
        }
        for (waiter, pid, _, wait_marker, takeover_marker) in &mut waiters {
            wait_for_file(wait_marker, waiter);
            let waiting: Value =
                serde_json::from_slice(&std::fs::read(wait_marker).unwrap()).unwrap();
            assert_eq!(waiting["point"], "waiting-for-owner-lock");
            assert_eq!(waiting["sidecar_process_id"], *pid);
            assert!(!takeover_marker.exists());
        }

        // SIGKILL releases operation.lock in the kernel. The already-blocked raw
        // caller, rather than a newly launched recovery process, must acquire it.
        owner.kill().unwrap();
        owner.wait().unwrap();
        let mut rollback_owner_pid = None;
        let mut terminal_owner_pid = None;
        let survivor_index = if point == "after_intent_queue_rename" {
            wait_for_file(&rollback_owner_marker, &mut waiters[0].0);
            let rollback_owner: Value =
                serde_json::from_slice(&std::fs::read(&rollback_owner_marker).unwrap()).unwrap();
            let rollback_index = waiters
                .iter()
                .position(|(_, pid, _, _, _)| {
                    rollback_owner["sidecar_process_id"].as_u64() == Some(u64::from(*pid))
                })
                .expect("rollback owner was not an already-waiting caller");
            assert!(waiters[rollback_index].4.exists());
            let blocked_indices = waiters
                .iter()
                .enumerate()
                .filter_map(|(index, _)| (index != rollback_index).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(blocked_indices.len(), 2);
            for index in &blocked_indices {
                assert!(
                    !waiters[*index].4.exists(),
                    "later waiter took over before rollback owner death"
                );
                assert!(waiters[*index].0.try_wait().unwrap().is_none());
            }
            let rollback_queue: Value =
                serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
            let rollback_row = rollback_queue["batches"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["batch_id"] == batch_id)
                .unwrap();
            assert_eq!(rollback_row["status"], "cancellation_pending");
            assert_eq!(
                rollback_row["payload"]["phase"],
                "renderer-rollback-durable"
            );
            assert_eq!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());
            assert_eq!(std::fs::read(&conflicts_path).unwrap(), original_conflicts);

            rollback_owner_pid = Some(waiters[rollback_index].1);
            waiters[rollback_index].0.kill().unwrap();
            waiters[rollback_index].0.wait().unwrap();
            let (killed_response, _, _) = waiters[rollback_index].2.take().unwrap().join().unwrap();
            assert!(
                killed_response.is_none(),
                "rollback owner published a response before its second SIGKILL"
            );

            wait_for_file(&terminal_owner_marker, &mut waiters[blocked_indices[0]].0);
            let terminal_owner: Value =
                serde_json::from_slice(&std::fs::read(&terminal_owner_marker).unwrap()).unwrap();
            let terminal_index = waiters
                .iter()
                .position(|(_, pid, _, _, _)| {
                    terminal_owner["sidecar_process_id"].as_u64() == Some(u64::from(*pid))
                })
                .expect("terminal publisher was not an already-waiting caller");
            assert_ne!(terminal_index, rollback_index);
            assert!(waiters[terminal_index].4.exists());
            let read_only_index = blocked_indices
                .into_iter()
                .find(|index| *index != terminal_index)
                .expect("missing third pre-existing waiter");
            assert!(
                !waiters[read_only_index].4.exists(),
                "third waiter took over before terminal publisher death"
            );
            assert!(waiters[read_only_index].0.try_wait().unwrap().is_none());
            let terminal_queue: Value =
                serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
            let terminal_row = terminal_queue["batches"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["batch_id"] == batch_id)
                .unwrap();
            assert_eq!(terminal_row["status"], "request_cancelled");
            assert_eq!(terminal_row["error_code"], -32800);
            assert_eq!(
                terminal_row["payload"]["phase"],
                "renderer-cancelled-takeover"
            );
            assert_eq!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());
            assert_eq!(std::fs::read(&conflicts_path).unwrap(), original_conflicts);

            terminal_owner_pid = Some(waiters[terminal_index].1);
            waiters[terminal_index].0.kill().unwrap();
            waiters[terminal_index].0.wait().unwrap();
            let (killed_response, _, _) = waiters[terminal_index].2.take().unwrap().join().unwrap();
            assert!(
                killed_response.is_none(),
                "terminal publisher returned before its third SIGKILL"
            );
            if let Some(compaction_point) = compaction_point {
                wait_for_file(&compaction_marker, &mut waiters[read_only_index].0);
                assert!(
                    !waiters[read_only_index].4.exists(),
                    "read-only waiter claimed cancellation ownership while compacting"
                );
                let compacting_queue: Value =
                    serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
                let compacting_rows = compacting_queue["batches"].as_array().unwrap();
                assert_eq!(
                    compacting_rows
                        .iter()
                        .map(|row| (
                            row["batch_id"].as_str().unwrap(),
                            row["status"].as_str().unwrap(),
                        ))
                        .collect::<Vec<_>>(),
                    fifo_heads
                        .iter()
                        .map(|(batch_id, _)| (*batch_id, "sidecar_committed"))
                        .chain(
                            (compaction_point == "after_archive_fsync")
                                .then_some((batch_id, "request_cancelled")),
                        )
                        .collect::<Vec<_>>(),
                    "pre-existing waiter changed the FIFO prefix at {compaction_point}"
                );
                std::fs::write(&compaction_release, b"release\n").unwrap();
            }
            read_only_index
        } else {
            0
        };
        let (mut waiter, waiter_pid, waiter_call, _wait_marker, takeover_marker) =
            waiters.swap_remove(survivor_index);
        let (waiter_response, mut waiter_in, mut waiter_out) = waiter_call.unwrap().join().unwrap();
        let waiter_response = waiter_response.expect("surviving waiter response");
        assert_eq!(
            waiter_response["error"],
            json!({"code": -32800, "message": "request cancelled"})
        );
        let mut protocol_error_codes = vec![waiter_response["error"]["code"].as_i64().unwrap()];
        if point == "after_intent_queue_rename" {
            assert!(
                !takeover_marker.exists(),
                "third waiter rewrote the already-published terminal"
            );
            if compaction_point.is_some() {
                assert!(compaction_marker.exists());
            }
        } else {
            let takeover: Value =
                serde_json::from_slice(&std::fs::read(&takeover_marker).unwrap()).unwrap();
            assert_eq!(takeover["point"], "took-over-pending-cancellation");
            assert_eq!(takeover["sidecar_process_id"], waiter_pid);
        }
        assert_eq!(
            waiters
                .iter()
                .filter(|(_, _, _, _, marker)| marker.exists())
                .count()
                + usize::from(takeover_marker.exists()),
            if point == "after_intent_queue_rename" {
                2
            } else {
                1
            },
            "each dead owner must be followed by exactly one waiting takeover"
        );
        assert_eq!(std::fs::read(&save_tmx).unwrap(), original_tmx.as_bytes());
        assert_eq!(std::fs::read(&conflicts_path).unwrap(), original_conflicts);
        let remaining_queue: Value =
            serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
        assert_eq!(
            remaining_queue["batches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["batch_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            fifo_heads
                .iter()
                .map(|(batch_id, _)| *batch_id)
                .collect::<Vec<_>>()
        );

        // Retry every killed logical caller with the same durable batch key.
        // The initial owner, rollback owner, terminal publisher, and the
        // surviving read-only waiter all settle at the protocol boundary as
        // -32800.
        let (mut owner_retry, mut retry_in, mut retry_out) =
            spawn_sidecar(&config, None, None, None, None, None);
        let owner_retry_response = rpc(
            &mut retry_in,
            &mut retry_out,
            6,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": "raw-cancel-owner",
                "owner_process_id": owner_retry.id(),
                "generation": generation,
                "batch_id": batch_id,
                "operation": "resolve-conflict",
                "outcome": "cancelled",
            }),
        );
        assert_eq!(
            owner_retry_response["error"],
            json!({"code": -32800, "message": "request cancelled"})
        );
        protocol_error_codes.push(owner_retry_response["error"]["code"].as_i64().unwrap());
        if let Some(rollback_owner_pid) = rollback_owner_pid {
            let rollback_owner_retry = rpc(
                &mut retry_in,
                &mut retry_out,
                7,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": "raw-cancel-waiter-retry",
                    "owner_process_id": rollback_owner_pid,
                    "generation": generation,
                    "batch_id": batch_id,
                    "operation": "resolve-conflict",
                    "outcome": "cancelled",
                }),
            );
            assert_eq!(
                rollback_owner_retry["error"],
                json!({"code": -32800, "message": "request cancelled"})
            );
            protocol_error_codes.push(rollback_owner_retry["error"]["code"].as_i64().unwrap());
        }
        if let Some(terminal_owner_pid) = terminal_owner_pid {
            let terminal_owner_retry = rpc(
                &mut retry_in,
                &mut retry_out,
                8,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": "raw-cancel-terminal-retry",
                    "owner_process_id": terminal_owner_pid,
                    "generation": generation,
                    "batch_id": batch_id,
                    "operation": "resolve-conflict",
                    "outcome": "cancelled",
                }),
            );
            assert_eq!(
                terminal_owner_retry["error"],
                json!({"code": -32800, "message": "request cancelled"})
            );
            protocol_error_codes.push(terminal_owner_retry["error"]["code"].as_i64().unwrap());
        }
        if point == "after_intent_queue_rename" {
            protocol_error_codes.sort_unstable();
            assert_eq!(
                protocol_error_codes,
                vec![-32800, -32800, -32800, -32800],
                "all four raw cancellation calls must converge on the protocol terminal"
            );
        }

        for (offset, (head_batch_id, operation)) in fifo_heads.iter().enumerate() {
            let pending = rpc(
                &mut retry_in,
                &mut retry_out,
                20 + offset as i64 * 2,
                "transaction.receipt.pending",
                json!({
                    "root": root,
                    "app_instance": "raw-fifo-owner",
                    "owner_process_id": owner_retry.id(),
                    "generation": generation,
                }),
            );
            assert_eq!(
                pending["result"]["envelopes"][0]["batch_id"],
                *head_batch_id
            );
            assert_ne!(
                pending["result"]["envelopes"][0]["batch_id"], batch_id,
                "cancelled resolve tail escaped into the FIFO prefix"
            );
            let acknowledged = rpc(
                &mut retry_in,
                &mut retry_out,
                21 + offset as i64 * 2,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": "raw-fifo-owner",
                    "owner_process_id": owner_retry.id(),
                    "generation": generation,
                    "batch_id": head_batch_id,
                    "operation": operation,
                    "outcome": "succeeded",
                }),
            );
            assert_eq!(acknowledged["result"]["ack"]["acknowledged"], true);
        }
        let history = std::fs::read_to_string(&history_path).unwrap();
        let rows = history
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["batch_id"] == batch_id)
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter()
                .filter(|row| {
                    row["status"] == "cancellation_pending"
                        && row["payload"]["phase"] == "renderer-rollback-durable"
                })
                .count(),
            1,
            "owner death opened more than one durable rollback checkpoint"
        );
        assert_eq!(
            rows.iter()
                .filter(|row| {
                    row["status"] == "request_cancelled"
                        && row["error_code"] == -32800
                        && row["payload"]["phase"] == "renderer-cancelled-takeover"
                })
                .count(),
            1,
            "owner death published more than one terminal cancellation"
        );
        let no_resolve_envelope = rpc(
            &mut retry_in,
            &mut retry_out,
            30,
            "transaction.receipt.pending",
            json!({
                "root": root,
                "app_instance": "raw-fifo-owner",
                "owner_process_id": owner_retry.id(),
                "generation": generation,
            }),
        );
        assert_eq!(no_resolve_envelope["result"]["envelopes"], json!([]));
        assert!(!active_path.exists());
        let responsive = rpc(&mut waiter_in, &mut waiter_out, 8, "sys.version", json!({}));
        assert_eq!(responsive["result"]["version"], "6.2.0");
        waiter.kill().unwrap();
        waiter.wait().unwrap();
        owner_retry.kill().unwrap();
        owner_retry.wait().unwrap();
    }
}

#[test]
fn project_open_recovers_only_its_interrupted_resolution_generation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("recover-team-resolve");
    let props = omegat_core::properties::ProjectProperties::create(
        root.clone(),
        "en".into(),
        "fr".into(),
        false,
    );
    props.ensure_dirs().unwrap();
    props.write().unwrap();
    std::fs::write(props.source_dir.join("source.txt"), "same source").unwrap();
    let ours_tmx = r#"<?xml version="1.0" encoding="UTF-8"?><tmx version="1.4"><body><tu><tuv xml:lang="en"><seg>same source</seg></tuv><tuv xml:lang="fr"><seg>ours</seg></tuv></tu></body></tmx>"#;
    std::fs::write(props.save_tmx_path(), ours_tmx).unwrap();
    let prep = root.join(".repositories/prep");
    std::fs::create_dir_all(&prep).unwrap();
    let conflicts = json!([
        {
            "kind": "tmx",
            "source": "same source",
            "ours": "ours",
            "theirs": "theirs",
            "message": "TMX conflict on same source"
        },
        {
            "kind": "glossary",
            "source": "pending glossary",
            "ours": "ours pending",
            "theirs": "theirs pending",
            "message": "glossary conflict on pending glossary"
        }
    ]);
    let conflicts_before = serde_json::to_vec_pretty(&conflicts).unwrap();
    std::fs::write(prep.join("conflicts.json"), &conflicts_before).unwrap();

    let transactions = root.join(".repositories/transactions");
    let snapshot = transactions.join("interrupted-resolution.snapshot");
    copy_product_tree(&root, &snapshot.join("project"));
    std::fs::create_dir_all(snapshot.join("prep")).unwrap();
    std::fs::write(snapshot.join("prep/conflicts.json"), &conflicts_before).unwrap();

    let partial_tmx = ours_tmx.replace("<seg>ours</seg>", "<seg>theirs</seg>");
    std::fs::write(props.save_tmx_path(), partial_tmx).unwrap();
    std::fs::write(prep.join("conflicts.json"), "[]").unwrap();
    std::fs::write(prep.join("resolved.json"), r#"["same source"]"#).unwrap();
    std::fs::create_dir_all(&transactions).unwrap();
    std::fs::write(
        transactions.join("active.json"),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "project_root": root,
            "generation": 8,
            "batch_id": "interrupted-resolution",
            "status": "pending",
            "error_code": null,
            "updated_unix_ms": 1,
            "payload": {
                "operation": "resolve-conflict",
                "phase": "mutating",
                "snapshot": snapshot,
                "prep_existed": true,
                "file_remotes": [],
                "repository_count": 0,
                "rollback_versions": [],
                "commit_started": [],
                "published": []
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let other_root = temp.path().join("new-project-generation");
    let other_props = omegat_core::properties::ProjectProperties::create(
        other_root.clone(),
        "en".into(),
        "fr".into(),
        false,
    );
    other_props.ensure_dirs().unwrap();
    other_props.write().unwrap();
    std::fs::write(other_props.source_dir.join("new.txt"), "new generation").unwrap();
    std::fs::write(
        other_props.save_tmx_path(),
        ours_tmx.replace("same source", "new generation"),
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let opened = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "project.open",
        json!({"root": root}),
    );
    assert_eq!(opened["result"]["root"], root.to_string_lossy().as_ref());
    assert_eq!(
        std::fs::read_to_string(props.save_tmx_path()).unwrap(),
        ours_tmx
    );
    assert_eq!(
        std::fs::read(prep.join("conflicts.json")).unwrap(),
        conflicts_before
    );
    assert!(!prep.join("resolved.json").exists());
    assert!(!transactions.join("active.json").exists());
    assert!(!snapshot.exists());
    let restored_conflicts = rpc(&mut stdin, &mut stdout, 2, "team.conflicts", json!({}));
    assert_eq!(
        restored_conflicts["result"]["conflicts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let opened_other = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "project.open",
        json!({"root": other_root}),
    );
    assert_eq!(
        opened_other["result"]["root"],
        other_root.to_string_lossy().as_ref()
    );
    assert_eq!(
        std::fs::read_to_string(other_props.save_tmx_path()).unwrap(),
        ours_tmx.replace("same source", "new generation")
    );
    assert!(!other_root
        .join(".repositories/transactions/active.json")
        .exists());
    let _ = child.kill();
}

#[test]
fn fingerprint_fifo_survives_sidecar_restarts_and_rejects_stale_projects() {
    fn create_project(root: &std::path::Path, source: &str) {
        let props = omegat_core::properties::ProjectProperties::create(
            root.to_path_buf(),
            "en".into(),
            "fr".into(),
            false,
        );
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        std::fs::write(props.source_dir.join("source.txt"), source).unwrap();
    }

    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("current-project");
    let other = temp.path().join("other-project");
    create_project(&root, "current source");
    create_project(&other, "other source");

    let (mut first_child, mut first_in, mut first_out) = spawn_sidecar(&config);
    let opened = rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.open",
        json!({ "root": root }),
    );
    assert!(opened["error"].is_null());
    let first = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-before-kill",
            "generation": 7,
            "paths": [root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "fingerprint-one" },
            "sources": ["native"]
        }),
    );
    let second = rpc(
        &mut first_in,
        &mut first_out,
        3,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-before-kill",
            "generation": 7,
            "paths": [root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "fingerprint-two" },
            "sources": ["sidecar"]
        }),
    );
    let first_id = first["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    let second_id = second["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(first_id, second_id);
    first_child.kill().unwrap();
    first_child.wait().unwrap();

    let (mut second_child, mut second_in, mut second_out) = spawn_sidecar(&config);
    rpc(
        &mut second_in,
        &mut second_out,
        4,
        "project.open",
        json!({ "root": root }),
    );
    let recovered = rpc(
        &mut second_in,
        &mut second_out,
        5,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1
        }),
    );
    assert_eq!(
        recovered["result"]["envelopes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|batch| batch["batch_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![first_id.as_str()]
    );
    let completed = rpc(
        &mut second_in,
        &mut second_out,
        6,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1,
            "batch_id": first_id,
            "operation": "project.external-refresh",
            "outcome": "cancelled"
        }),
    );
    assert_eq!(completed["result"]["ack"]["acknowledged"], true);
    second_child.kill().unwrap();
    second_child.wait().unwrap();

    let (mut third_child, mut third_in, mut third_out) = spawn_sidecar(&config);
    rpc(
        &mut third_in,
        &mut third_out,
        7,
        "project.open",
        json!({ "root": root }),
    );
    let still_pending = rpc(
        &mut third_in,
        &mut third_out,
        8,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1
        }),
    );
    assert_eq!(
        still_pending["result"]["envelopes"][0]["batch_id"],
        second_id.as_str()
    );
    rpc(
        &mut third_in,
        &mut third_out,
        9,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1,
            "batch_id": second_id,
            "operation": "project.external-refresh",
            "outcome": "coalesced"
        }),
    );
    let completed_stays_gone = rpc(
        &mut third_in,
        &mut third_out,
        10,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1
        }),
    );
    assert_eq!(completed_stays_gone["result"]["envelopes"], json!([]));

    rpc(
        &mut third_in,
        &mut third_out,
        11,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 1,
            "paths": [root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "stale-generation" },
            "sources": ["native"]
        }),
    );
    let stale_generation = rpc(
        &mut third_in,
        &mut third_out,
        12,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 2
        }),
    );
    assert_eq!(stale_generation["result"]["envelopes"], json!([]));

    rpc(
        &mut third_in,
        &mut third_out,
        13,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 2,
            "paths": [root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "wrong-project" },
            "sources": ["sidecar"]
        }),
    );
    rpc(
        &mut third_in,
        &mut third_out,
        14,
        "project.open",
        json!({ "root": other }),
    );
    let other_pending = rpc(
        &mut third_in,
        &mut third_out,
        15,
        "transaction.receipt.pending",
        json!({
            "root": other,
            "app_instance": "electron-after-kill",
            "generation": 3
        }),
    );
    assert_eq!(other_pending["result"]["envelopes"], json!([]));
    rpc(
        &mut third_in,
        &mut third_out,
        16,
        "project.open",
        json!({ "root": root }),
    );
    let old_root_does_not_revive = rpc(
        &mut third_in,
        &mut third_out,
        17,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-kill",
            "generation": 4
        }),
    );
    assert_eq!(old_root_does_not_revive["result"]["envelopes"], json!([]));
    let _ = third_child.kill();
}

#[cfg(target_os = "linux")]
#[test]
fn keep_ours_resolve_receipt_survivors_elect_third_owner_after_two_deaths() {
    struct Sidecar {
        child: std::process::Child,
        input: std::process::ChildStdin,
        output: BufReader<std::process::ChildStdout>,
    }

    fn spawn_sidecar(
        config: &std::path::Path,
        owner_marker: Option<(&std::path::Path, &std::path::Path)>,
        wait_marker: Option<&std::path::Path>,
    ) -> Sidecar {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        if let Some((marker, release)) = owner_marker {
            command
                .env("OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_MARKER", marker)
                .env(
                    "OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_RELEASE",
                    release,
                );
        }
        if let Some(marker) = wait_marker {
            command.env("OMEGAT_TEST_TRANSACTION_OWNER_RETRY_WAIT_MARKER", marker);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Sidecar {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn wait_for_file(path: &std::path::Path, children: &mut [Sidecar]) {
        for _ in 0..7_000 {
            for sidecar in children.iter_mut() {
                assert!(
                    sidecar.child.try_wait().unwrap().is_none(),
                    "replacement exited before checkpoint {}",
                    path.display()
                );
            }
            if path.is_file() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {}", path.display());
    }

    fn send_pending(
        sidecar: &mut Sidecar,
        id: i64,
        root: &std::path::Path,
        app_instance: &str,
        generation: u64,
        owner_retry_timeout_ms: Option<u64>,
        owner_retry_attempts: Option<u64>,
    ) {
        let mut params = json!({
            "root": root,
            "app_instance": app_instance,
            "generation": generation,
        });
        if let Some(timeout) = owner_retry_timeout_ms {
            params["owner_retry_timeout_ms"] = json!(timeout);
        }
        if let Some(attempts) = owner_retry_attempts {
            params["owner_retry_attempts"] = json!(attempts);
        }
        writeln!(
            sidecar.input,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "transaction.receipt.pending",
                "params": params,
            })
        )
        .unwrap();
        sidecar.input.flush().unwrap();
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("resolve-project");
    let remote = temp.path().join("file-remote");
    let remote_tmx = remote.join("omegat/project_save.tmx");
    let remote_marker = remote.join("mapping.marker");
    let active_path = root.join(".repositories/transactions/active.json");
    let owner_path = root.join(".repositories/transactions/renderer-owner.json");
    let history_path = root.join(".repositories/transactions/history.ndjson");
    let batch_id = "resolve-retry-head";

    let mut setup = spawn_sidecar(&config, None, None);
    rpc(
        &mut setup.input,
        &mut setup.output,
        1,
        "project.create",
        json!({
            "root": root,
            "source_lang": "en",
            "target_lang": "fr",
            "sentence_seg": false,
        }),
    );
    std::fs::write(root.join("source/a-wanted.txt"), "Repeated resolve source").unwrap();
    std::fs::write(root.join("source/z-decoy.txt"), "Repeated resolve source").unwrap();
    rpc(
        &mut setup.input,
        &mut setup.output,
        2,
        "project.reload",
        json!({}),
    );
    let entries = rpc(
        &mut setup.input,
        &mut setup.output,
        3,
        "entry.list",
        json!({}),
    );
    let wanted = entries["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"]["file"] == "a-wanted.txt")
        .unwrap()
        .clone();
    let decoy = entries["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"]["file"] == "z-decoy.txt")
        .unwrap()
        .clone();
    let mut key_fields = wanted["key"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    key_fields.sort_unstable();
    assert_eq!(
        key_fields,
        ["file", "id", "next", "path", "prev", "source_text"]
    );
    rpc(
        &mut setup.input,
        &mut setup.output,
        4,
        "entry.set",
        json!({
            "index": wanted["index"],
            "key": wanted["key"],
            "translation": "resolve base",
            "revision": wanted["revision"],
            "default_translation": false,
        }),
    );
    rpc(
        &mut setup.input,
        &mut setup.output,
        5,
        "project.save",
        json!({}),
    );
    std::fs::create_dir_all(remote_tmx.parent().unwrap()).unwrap();
    std::fs::copy(root.join("omegat/project_save.tmx"), &remote_tmx).unwrap();
    std::fs::write(&remote_marker, "file mapping must remain exact").unwrap();
    let mapped = rpc(
        &mut setup.input,
        &mut setup.output,
        6,
        "team.mapping",
        json!({
            "repositories": [{
                "repo_type": "file",
                "url": remote,
                "branch": null,
                "mappings": [
                    {
                        "local": "/omegat/project_save.tmx",
                        "repository": "/omegat/project_save.tmx",
                        "includes": [],
                        "excludes": [],
                    },
                    {
                        "local": "/team-mapping.marker",
                        "repository": "/mapping.marker",
                        "includes": [],
                        "excludes": [],
                    },
                ],
            }],
        }),
    );
    assert_eq!(mapped["result"]["ok"], true);
    let initial_sync = rpc(
        &mut setup.input,
        &mut setup.output,
        7,
        "team.sync",
        json!({}),
    );
    assert_eq!(initial_sync["result"]["action"], "sync");

    let current = rpc(
        &mut setup.input,
        &mut setup.output,
        8,
        "entry.list",
        json!({}),
    );
    let current_wanted = current["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == wanted["key"])
        .unwrap();
    rpc(
        &mut setup.input,
        &mut setup.output,
        9,
        "entry.set",
        json!({
            "index": current_wanted["index"],
            "key": current_wanted["key"],
            "translation": "resolve ours",
            "revision": current_wanted["revision"],
            "default_translation": false,
        }),
    );
    rpc(
        &mut setup.input,
        &mut setup.output,
        10,
        "project.save",
        json!({}),
    );
    let remote_tmx_text = std::fs::read_to_string(&remote_tmx).unwrap();
    assert!(remote_tmx_text.contains("<seg>resolve base</seg>"));
    std::fs::write(
        &remote_tmx,
        remote_tmx_text.replace("<seg>resolve base</seg>", "<seg>resolve theirs</seg>"),
    )
    .unwrap();
    let conflict_sync = rpc(
        &mut setup.input,
        &mut setup.output,
        11,
        "team.sync",
        json!({}),
    );
    assert_eq!(conflict_sync["error"]["code"], -32005);
    let conflicts = rpc(
        &mut setup.input,
        &mut setup.output,
        12,
        "team.conflicts",
        json!({}),
    );
    assert_eq!(
        conflicts["result"]["conflicts"].as_array().unwrap().len(),
        1
    );
    let conflict = &conflicts["result"]["conflicts"][0];
    assert_eq!(conflict["entry_key"], wanted["key"]);
    assert_eq!(conflict["ours"], "resolve ours");
    assert_eq!(conflict["theirs"], "resolve theirs");

    let resolved = rpc(
        &mut setup.input,
        &mut setup.output,
        13,
        "team.resolve",
        json!({
            "source": "Repeated resolve source",
            "rebind_key": wanted["key"],
            "side": "ours",
            "transaction_project_root": root,
            "transaction_generation": 70,
            "transaction_batch_id": batch_id,
        }),
    );
    assert_eq!(resolved["result"]["conflicts"], json!([]));
    assert_eq!(resolved["result"]["receipt"]["batch_id"], batch_id);
    assert_eq!(
        resolved["result"]["receipt"]["payload"]["operation"],
        "resolve-conflict"
    );
    assert_eq!(resolved["result"]["receipt"]["status"], "sidecar_committed");
    let tmx_path = root.join("omegat/project_save.tmx");
    let tmx_after_resolve = std::fs::read(&tmx_path).unwrap();
    let tmx_mtime_after_resolve = std::fs::metadata(&tmx_path).unwrap().modified().unwrap();
    let remote_after_resolve = std::fs::read(&remote_marker).unwrap();
    let remote_mtime_after_resolve = std::fs::metadata(&remote_marker)
        .unwrap()
        .modified()
        .unwrap();

    let old_pending = rpc(
        &mut setup.input,
        &mut setup.output,
        14,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "resolve-old-owner",
            "generation": 71,
        }),
    );
    assert_eq!(old_pending["result"]["envelopes"][0]["batch_id"], batch_id);
    let old_owner: Value = serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
    let old_pid = setup.child.id();
    assert_eq!(old_owner["process_id"], old_pid);
    setup.child.kill().unwrap();
    assert!(!setup.child.wait().unwrap().success());
    assert!(!std::path::Path::new("/proc")
        .join(old_pid.to_string())
        .exists());

    let owner_markers = (0..4)
        .map(|index| temp.path().join(format!("resolve-owner-{index}.json")))
        .collect::<Vec<_>>();
    let owner_releases = (0..4)
        .map(|index| temp.path().join(format!("resolve-owner-{index}.release")))
        .collect::<Vec<_>>();
    let wait_markers = (0..4)
        .map(|index| temp.path().join(format!("resolve-wait-{index}.json")))
        .collect::<Vec<_>>();
    let mut replacements = (0..4)
        .map(|index| {
            spawn_sidecar(
                &config,
                Some((&owner_markers[index], &owner_releases[index])),
                Some(&wait_markers[index]),
            )
        })
        .collect::<Vec<_>>();
    for (index, replacement) in replacements.iter_mut().enumerate() {
        send_pending(
            replacement,
            1,
            &root,
            &format!("resolve-replacement-{index}"),
            80 + index as u64,
            Some(60_000),
            Some(2),
        );
    }
    let first_winner_index = {
        let mut winner = None;
        for _ in 0..7_000 {
            for replacement in &mut replacements {
                assert!(
                    replacement.child.try_wait().unwrap().is_none(),
                    "replacement exited before the first owner election"
                );
            }
            for (index, marker) in owner_markers.iter().enumerate() {
                if marker.is_file() {
                    winner = Some(index);
                    break;
                }
            }
            if winner.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        winner.expect("first three-way resolve replacement election had no winner")
    };
    for index in (0..4).filter(|index| *index != first_winner_index) {
        wait_for_file(&wait_markers[index], &mut replacements);
    }
    assert!(!wait_markers[first_winner_index].exists());
    let first_owner: Value = serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
    let first_winner_pid = replacements[first_winner_index].child.id();
    assert_eq!(first_owner["process_id"], first_winner_pid);
    assert_ne!(first_owner["claim_id"], old_owner["claim_id"]);
    for index in (0..4).filter(|index| *index != first_winner_index) {
        let wait: Value =
            serde_json::from_slice(&std::fs::read(&wait_markers[index]).unwrap()).unwrap();
        assert_eq!(
            wait["previous_owner_process_id"], first_winner_pid,
            "replacement {index} waited on a stale owner claim"
        );
    }
    let queue_before_first_kill = std::fs::read(&active_path).unwrap();
    replacements[first_winner_index].child.kill().unwrap();
    assert!(!replacements[first_winner_index]
        .child
        .wait()
        .unwrap()
        .success());
    assert!(!std::path::Path::new("/proc")
        .join(first_winner_pid.to_string())
        .exists());
    assert_eq!(
        std::fs::read(&active_path).unwrap(),
        queue_before_first_kill,
        "first resolve winner changed the durable queue before delivery"
    );

    let surviving_indices = (0..4)
        .filter(|index| *index != first_winner_index)
        .collect::<Vec<_>>();
    let second_winner_index = {
        let mut winner = None;
        for _ in 0..7_000 {
            for index in &surviving_indices {
                assert!(
                    replacements[*index].child.try_wait().unwrap().is_none(),
                    "surviving replacement exited before the second owner election"
                );
                if owner_markers[*index].is_file() {
                    winner = Some(*index);
                    break;
                }
            }
            if winner.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        winner.expect("surviving resolve loser did not retry the owner election")
    };
    let second_loser_indices = surviving_indices
        .iter()
        .copied()
        .filter(|index| *index != second_winner_index)
        .collect::<Vec<_>>();
    let second_owner: Value = serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
    let second_winner_pid = replacements[second_winner_index].child.id();
    assert_eq!(second_owner["process_id"], second_winner_pid);
    assert_ne!(second_owner["claim_id"], first_owner["claim_id"]);
    let second_wait_markers = second_loser_indices
        .iter()
        .map(|index| {
            std::path::PathBuf::from(format!(
                "{}.{second_winner_pid}",
                wait_markers[*index].to_string_lossy()
            ))
        })
        .collect::<Vec<_>>();
    for _ in 0..7_000 {
        for index in &surviving_indices {
            assert!(
                replacements[*index].child.try_wait().unwrap().is_none(),
                "surviving replacement exited before second owner wait"
            );
        }
        if second_wait_markers.iter().all(|marker| marker.is_file()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        second_wait_markers.iter().all(|marker| marker.is_file()),
        "second-election losers did not wait on the second owner"
    );
    for marker in &second_wait_markers {
        let wait: Value = serde_json::from_slice(&std::fs::read(marker).unwrap()).unwrap();
        assert_eq!(wait["previous_owner_process_id"], second_winner_pid);
    }
    let queue_before_second_kill = std::fs::read(&active_path).unwrap();
    replacements[second_winner_index].child.kill().unwrap();
    assert!(!replacements[second_winner_index]
        .child
        .wait()
        .unwrap()
        .success());
    assert!(!std::path::Path::new("/proc")
        .join(second_winner_pid.to_string())
        .exists());
    assert_eq!(
        std::fs::read(&active_path).unwrap(),
        queue_before_second_kill,
        "second resolve winner changed the durable queue before delivery"
    );

    let third_winner_index = {
        let mut winner = None;
        for _ in 0..7_000 {
            for index in &second_loser_indices {
                assert!(
                    replacements[*index].child.try_wait().unwrap().is_none(),
                    "surviving replacement exited before the third owner election"
                );
                if owner_markers[*index].is_file() {
                    winner = Some(*index);
                    break;
                }
            }
            if winner.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        winner.expect("second resolve winner death did not trigger a third election")
    };
    let third_loser_index = *second_loser_indices
        .iter()
        .find(|index| **index != third_winner_index)
        .unwrap();
    let third_owner: Value = serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
    assert_eq!(
        third_owner["process_id"],
        replacements[third_winner_index].child.id()
    );
    assert_ne!(third_owner["claim_id"], second_owner["claim_id"]);
    let rejected_retry = response_for(&mut replacements[third_loser_index].output, 1);
    assert_eq!(rejected_retry["error"]["code"], -32005);
    assert!(rejected_retry["error"]["message"]
        .as_str()
        .unwrap()
        .contains(&format!(
            "replacement retry after owner pid {second_winner_pid} exited was rejected"
        )));

    std::fs::write(&owner_releases[third_winner_index], b"release\n").unwrap();
    let recovered = response_for(&mut replacements[third_winner_index].output, 1);
    assert_eq!(recovered["result"]["envelopes"][0]["batch_id"], batch_id);
    assert_eq!(
        recovered["result"]["owner_retry"]["previous_owner_process_id"],
        second_winner_pid
    );
    assert_eq!(
        recovered["result"]["owner_retry"]["previous_owner_process_ids"],
        json!([first_winner_pid, second_winner_pid])
    );
    let third_app = format!("resolve-replacement-{third_winner_index}");
    let third_generation = 80 + third_winner_index as u64;
    let third_winner = &mut replacements[third_winner_index];
    let ack = rpc(
        &mut third_winner.input,
        &mut third_winner.output,
        2,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": third_app,
            "generation": third_generation,
            "batch_id": batch_id,
            "operation": "resolve-conflict",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(ack["result"]["ack"]["acknowledged"], true);
    let opened = rpc(
        &mut third_winner.input,
        &mut third_winner.output,
        3,
        "project.open",
        json!({ "root": root }),
    );
    assert_eq!(opened["error"], Value::Null);
    let drained = rpc(
        &mut third_winner.input,
        &mut third_winner.output,
        4,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": third_app,
            "generation": third_generation,
        }),
    );
    assert_eq!(drained["result"]["envelopes"], json!([]));
    let final_entries = rpc(
        &mut third_winner.input,
        &mut third_winner.output,
        5,
        "entry.list",
        json!({}),
    );
    let final_wanted = final_entries["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == wanted["key"])
        .unwrap();
    let final_decoy = final_entries["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == decoy["key"])
        .unwrap();
    assert_eq!(final_wanted["translation"], "resolve ours");
    assert_eq!(final_decoy["translation"], "");
    assert_eq!(std::fs::read(&tmx_path).unwrap(), tmx_after_resolve);
    assert_eq!(
        std::fs::metadata(&tmx_path).unwrap().modified().unwrap(),
        tmx_mtime_after_resolve
    );
    assert_eq!(std::fs::read(&remote_marker).unwrap(), remote_after_resolve);
    assert_eq!(
        std::fs::metadata(&remote_marker)
            .unwrap()
            .modified()
            .unwrap(),
        remote_mtime_after_resolve
    );
    let history = std::fs::read_to_string(&history_path).unwrap();
    assert_eq!(
        history
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| {
                row["batch_id"] == batch_id
                    && row["status"] == "completed"
                    && row["payload"]["phase"] == "renderer-acknowledged"
            })
            .count(),
        1
    );
    for (index, replacement) in replacements.iter_mut().enumerate() {
        if index == first_winner_index || index == second_winner_index {
            continue;
        }
        replacement.child.kill().unwrap();
        replacement.child.wait().unwrap();
    }
}

#[test]
fn product_journal_compaction_survives_archive_and_queue_rename_interruptions() {
    fn spawn_sidecar(
        config: &std::path::Path,
        compaction_checkpoint: Option<(&str, &std::path::Path)>,
        owner_checkpoint: Option<(&std::path::Path, &std::path::Path)>,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        if let Some((point, marker)) = compaction_checkpoint {
            let point = match point {
                "archive" => "after_archive_fsync",
                "queue-rename" => "after_queue_rename",
                other => panic!("unknown product compaction checkpoint {other}"),
            };
            command
                .env("OMEGAT_TEST_PRODUCT_COMPACTION_POINT", point)
                .env("OMEGAT_TEST_PRODUCT_COMPACTION_MARKER", marker);
        }
        if let Some((marker, release)) = owner_checkpoint {
            command
                .env("OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_MARKER", marker)
                .env(
                    "OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_RELEASE",
                    release,
                );
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        (child, input, output)
    }

    fn wait_for_marker(path: &std::path::Path, child: &mut std::process::Child) {
        for _ in 0..1_000 {
            assert!(
                child.try_wait().unwrap().is_none(),
                "compaction owner exited before checkpoint {}",
                path.display()
            );
            if path.is_file() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "timed out waiting for compaction checkpoint {}",
            path.display()
        );
    }

    fn receipt_scope(root: &std::path::Path, app: &str, generation: u64) -> Value {
        json!({
            "root": root,
            "app_instance": app,
            "generation": generation,
        })
    }

    for point in ["archive", "queue-rename"] {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join(format!("product-{point}"));
        let active_path = root.join(".repositories/transactions/active.json");
        let owner_path = root.join(".repositories/transactions/renderer-owner.json");
        let history_path = root.join(".repositories/transactions/history.ndjson");
        let marker_path = temp.path().join(format!("product-{point}-checkpoint"));
        let save_tmx = root.join("omegat/project_save.tmx");
        let receipt_batch = format!("product-{point}-receipt");
        let tail_batch = format!("product-{point}-save-tail");
        let terminal_batch = format!("product-{point}-acknowledged-terminal");

        let (mut setup, mut setup_in, mut setup_out) = spawn_sidecar(&config, None, None);
        rpc(
            &mut setup_in,
            &mut setup_out,
            1,
            "project.create",
            json!({
                "root": root,
                "source_lang": "en",
                "target_lang": "fr",
                "sentence_seg": false,
            }),
        );
        std::fs::write(
            root.join("source/source.txt"),
            format!("product {point} source"),
        )
        .unwrap();
        rpc(
            &mut setup_in,
            &mut setup_out,
            2,
            "project.reload",
            json!({}),
        );
        let entries = rpc(&mut setup_in, &mut setup_out, 3, "entry.list", json!({}));
        let entry = &entries["result"][0];
        let mut key_fields = entry["key"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        key_fields.sort_unstable();
        assert_eq!(
            key_fields,
            ["file", "id", "next", "path", "prev", "source_text"]
        );
        let committed = rpc(
            &mut setup_in,
            &mut setup_out,
            4,
            "entry.set",
            json!({
                "index": entry["index"],
                "key": entry["key"],
                "translation": format!("product {point} translation"),
                "note": "product compaction contract",
                "revision": entry["revision"],
                "default_translation": false,
                "transaction_project_root": root,
                "transaction_generation": 8,
                "transaction_batch_id": receipt_batch,
            }),
        );
        assert_eq!(
            committed["result"]["receipt"]["status"],
            "sidecar_committed"
        );
        let saved = rpc(
            &mut setup_in,
            &mut setup_out,
            5,
            "project.save",
            json!({
                "transaction_project_root": root,
                "transaction_generation": 8,
                "transaction_batch_id": tail_batch,
            }),
        );
        assert_eq!(
            saved["result"]["receipt"]["payload"]["operation"],
            "project.save"
        );
        std::thread::sleep(Duration::from_millis(5));
        let refresh = rpc(
            &mut setup_in,
            &mut setup_out,
            6,
            "project.refresh.enqueue",
            json!({
                "root": root,
                "app_instance": format!("setup-{point}"),
                "generation": 8,
                "paths": [root.join("source/source.txt")],
                "fingerprints": { "source/source.txt": format!("{point}-refresh-tail") },
                "sources": ["native"],
            }),
        );
        let refresh_batch = refresh["result"]["batch"]["batch_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(refresh["result"]["batch"]["status"], "pending");
        setup.kill().unwrap();
        setup.wait().unwrap();

        let mut journal: Value =
            serde_json::from_slice(&std::fs::read(&active_path).unwrap()).unwrap();
        assert_eq!(journal["version"], 2);
        assert_eq!(journal["batches"].as_array().unwrap().len(), 3);
        let mut terminal = journal["batches"][0].clone();
        terminal["batch_id"] = json!(terminal_batch);
        terminal["status"] = json!("completed");
        terminal["updated_unix_ms"] = json!(1);
        terminal["payload"]["phase"] = json!("renderer-acknowledged");
        let terminal_snapshot = root
            .join(".repositories/transactions")
            .join(format!("{terminal_batch}.snapshot"));
        std::fs::create_dir_all(&terminal_snapshot).unwrap();
        std::fs::write(terminal_snapshot.join("archived"), b"terminal").unwrap();
        terminal["payload"]["snapshot"] = json!(terminal_snapshot);
        journal["batches"]
            .as_array_mut()
            .unwrap()
            .insert(0, terminal);
        std::fs::write(&active_path, serde_json::to_vec_pretty(&journal).unwrap()).unwrap();
        let original_queue = std::fs::read(&active_path).unwrap();
        let tmx_before = std::fs::read(&save_tmx).unwrap();
        let tmx_mtime_before = std::fs::metadata(&save_tmx).unwrap().modified().unwrap();

        let (mut interrupted, mut interrupted_in, mut interrupted_out) =
            spawn_sidecar(&config, Some((point, &marker_path)), None);
        rpc(
            &mut interrupted_in,
            &mut interrupted_out,
            7,
            "project.open",
            json!({ "root": root }),
        );
        writeln!(
            interrupted_in,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": 8,
                "method": "transaction.receipt.pending",
                "params": receipt_scope(&root, &format!("interrupted-{point}"), 9),
            })
        )
        .unwrap();
        interrupted_in.flush().unwrap();
        wait_for_marker(&marker_path, &mut interrupted);

        let live_owner: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(live_owner["app_instance"], format!("interrupted-{point}"));
        assert_eq!(live_owner["process_id"], interrupted.id());
        assert_eq!(live_owner["generation"], 9);
        let queue_at_checkpoint_bytes = std::fs::read(&active_path).unwrap();
        let queue_at_checkpoint: Value =
            serde_json::from_slice(&queue_at_checkpoint_bytes).unwrap();
        let expected_batches = if point == "archive" {
            vec![
                terminal_batch.as_str(),
                receipt_batch.as_str(),
                tail_batch.as_str(),
                refresh_batch.as_str(),
            ]
        } else {
            vec![
                receipt_batch.as_str(),
                tail_batch.as_str(),
                refresh_batch.as_str(),
            ]
        };
        assert_eq!(
            queue_at_checkpoint["batches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["batch_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_batches
        );
        if point == "archive" {
            assert_eq!(std::fs::read(&active_path).unwrap(), original_queue);
        }
        let archived_at_checkpoint = std::fs::read_to_string(&history_path).unwrap();
        assert_eq!(
            archived_at_checkpoint
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| { row["batch_id"] == terminal_batch && row["status"] == "completed" })
                .count(),
            1
        );

        let (mut contender, mut contender_in, mut contender_out) =
            spawn_sidecar(&config, None, None);
        let contender_pending = rpc(
            &mut contender_in,
            &mut contender_out,
            9,
            "transaction.receipt.pending",
            receipt_scope(&root, &format!("contender-{point}"), 10),
        );
        assert_eq!(contender_pending["error"]["code"], -32005);
        assert!(contender_pending["error"]["message"]
            .as_str()
            .unwrap()
            .contains("locked by another process"));
        assert_eq!(contender_pending["result"], Value::Null);
        let contender_ack = rpc(
            &mut contender_in,
            &mut contender_out,
            10,
            "transaction.receipt.ack",
            json!({
                "root": root,
                "app_instance": format!("contender-{point}"),
                "generation": 10,
                "batch_id": receipt_batch,
                "operation": "entry.set",
                "outcome": "succeeded",
            }),
        );
        assert_eq!(contender_ack["error"]["code"], -32005);
        assert!(contender_ack["error"]["message"]
            .as_str()
            .unwrap()
            .contains("locked by another process"));
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&owner_path).unwrap()).unwrap(),
            live_owner,
            "rejected contender replaced the live durable owner"
        );
        assert_eq!(
            std::fs::read(&active_path).unwrap(),
            queue_at_checkpoint_bytes,
            "rejected contender changed the product queue at {point}"
        );
        contender.kill().unwrap();
        contender.wait().unwrap();
        assert!(
            interrupted.try_wait().unwrap().is_none(),
            "checkpoint owner exited before the external kill"
        );
        let interrupted_pid = interrupted.id();
        interrupted.kill().unwrap();
        assert!(
            !interrupted.wait().unwrap().success(),
            "checkpoint owner unexpectedly exited successfully"
        );
        drop(interrupted_in);

        let dead_owner: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        assert_eq!(dead_owner["process_id"], interrupted_pid);
        assert!(
            !std::path::Path::new("/proc")
                .join(interrupted_pid.to_string())
                .exists(),
            "checkpoint owner PID remained live before the replacement election"
        );

        let first_claim_marker = temp.path().join(format!("{point}-first-claim"));
        let first_claim_release = temp.path().join(format!("{point}-first-release"));
        let mut first_wave = (0..3)
            .map(|index| {
                let app_instance = format!("replacement-{point}-first-{index}");
                let generation = (20 + index) as u64;
                let (child, mut input, mut output) = spawn_sidecar(
                    &config,
                    None,
                    Some((&first_claim_marker, &first_claim_release)),
                );
                rpc(
                    &mut input,
                    &mut output,
                    8,
                    "project.open",
                    json!({ "root": root }),
                );
                (child, input, output, app_instance, generation)
            })
            .collect::<Vec<_>>();
        for replacement in &mut first_wave {
            writeln!(
                replacement.1,
                "{}",
                json!({
                    "jsonrpc": "2.0",
                    "id": 9,
                    "method": "transaction.receipt.pending",
                    "params": receipt_scope(&root, &replacement.3, replacement.4),
                })
            )
            .unwrap();
            replacement.1.flush().unwrap();
        }
        for _ in 0..1_000 {
            for replacement in &mut first_wave {
                assert!(
                    replacement.0.try_wait().unwrap().is_none(),
                    "first-wave replacement exited before owner election at {point}"
                );
            }
            if first_claim_marker.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            first_claim_marker.is_file(),
            "first-wave owner checkpoint was not reached at {point}"
        );
        let first_claim: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        let first_winner_pid = first_claim["process_id"].as_u64().unwrap() as u32;
        let first_winner_index = first_wave
            .iter()
            .position(|replacement| replacement.0.id() == first_winner_pid)
            .expect("first-wave winner must be one of three simultaneous replacements");
        assert_ne!(first_claim["claim_id"], dead_owner["claim_id"]);
        assert_eq!(
            first_claim["app_instance"],
            first_wave[first_winner_index].3
        );
        assert_eq!(first_claim["generation"], first_wave[first_winner_index].4);
        let first_checkpoint: Value =
            serde_json::from_slice(&std::fs::read(&first_claim_marker).unwrap()).unwrap();
        assert_eq!(first_checkpoint["process_id"], first_winner_pid);
        for (index, replacement) in first_wave.iter_mut().enumerate() {
            if index == first_winner_index {
                continue;
            }
            let rejected = response_for(&mut replacement.2, 9);
            assert_eq!(rejected["error"]["code"], -32005);
            assert_eq!(rejected["result"], Value::Null);
        }
        for (index, replacement) in first_wave.iter_mut().enumerate() {
            if index == first_winner_index {
                continue;
            }
            replacement.0.kill().unwrap();
            replacement.0.wait().unwrap();
        }
        first_wave[first_winner_index].0.kill().unwrap();
        assert!(
            !first_wave[first_winner_index].0.wait().unwrap().success(),
            "first elected replacement unexpectedly exited successfully"
        );
        assert!(
            !std::path::Path::new("/proc")
                .join(first_winner_pid.to_string())
                .exists(),
            "first elected owner PID remained live before second election"
        );
        assert_eq!(
            std::fs::read(&active_path).unwrap(),
            queue_at_checkpoint_bytes,
            "first elected owner changed the queue before returning its head at {point}"
        );

        let second_claim_marker = temp.path().join(format!("{point}-second-claim"));
        let second_claim_release = temp.path().join(format!("{point}-second-release"));
        let mut second_wave = (0..3)
            .map(|index| {
                let app_instance = format!("replacement-{point}-second-{index}");
                let generation = (30 + index) as u64;
                let (child, mut input, mut output) = spawn_sidecar(
                    &config,
                    None,
                    Some((&second_claim_marker, &second_claim_release)),
                );
                rpc(
                    &mut input,
                    &mut output,
                    10,
                    "project.open",
                    json!({ "root": root }),
                );
                (child, input, output, app_instance, generation)
            })
            .collect::<Vec<_>>();
        for replacement in &mut second_wave {
            writeln!(
                replacement.1,
                "{}",
                json!({
                    "jsonrpc": "2.0",
                    "id": 11,
                    "method": "transaction.receipt.pending",
                    "params": receipt_scope(&root, &replacement.3, replacement.4),
                })
            )
            .unwrap();
            replacement.1.flush().unwrap();
        }
        for _ in 0..1_000 {
            for replacement in &mut second_wave {
                assert!(
                    replacement.0.try_wait().unwrap().is_none(),
                    "second-wave replacement exited before owner election at {point}"
                );
            }
            if second_claim_marker.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            second_claim_marker.is_file(),
            "second-wave owner checkpoint was not reached at {point}"
        );
        let replacement_owner: Value =
            serde_json::from_slice(&std::fs::read(&owner_path).unwrap()).unwrap();
        let second_winner_pid = replacement_owner["process_id"].as_u64().unwrap() as u32;
        let second_winner_index = second_wave
            .iter()
            .position(|replacement| replacement.0.id() == second_winner_pid)
            .expect("second-wave winner must be one of three simultaneous replacements");
        assert_ne!(replacement_owner["claim_id"], first_claim["claim_id"]);
        assert_eq!(
            replacement_owner["app_instance"],
            second_wave[second_winner_index].3
        );
        assert_eq!(
            replacement_owner["generation"],
            second_wave[second_winner_index].4
        );
        for (index, replacement) in second_wave.iter_mut().enumerate() {
            if index == second_winner_index {
                continue;
            }
            let rejected = response_for(&mut replacement.2, 11);
            assert_eq!(rejected["error"]["code"], -32005);
            assert_eq!(rejected["result"], Value::Null);
        }
        std::fs::write(&second_claim_release, b"release\n").unwrap();
        let recovered_head = response_for(&mut second_wave[second_winner_index].2, 11);
        assert_eq!(
            recovered_head["result"]["envelopes"][0]["batch_id"],
            receipt_batch
        );
        assert_eq!(
            recovered_head["result"]["envelopes"][0]["status"],
            "sidecar_committed"
        );
        let replacement_app = second_wave[second_winner_index].3.clone();
        let replacement_generation = second_wave[second_winner_index].4;

        let first_ack = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                12,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": replacement_app,
                    "generation": replacement_generation,
                    "batch_id": receipt_batch,
                    "operation": "entry.set",
                    "outcome": "succeeded",
                }),
            )
        };
        assert_eq!(first_ack["result"]["ack"]["acknowledged"], true);
        let recovered_tail = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                13,
                "transaction.receipt.pending",
                receipt_scope(&root, &replacement_app, replacement_generation),
            )
        };
        assert_eq!(
            recovered_tail["result"]["envelopes"][0]["batch_id"],
            tail_batch
        );
        assert_eq!(
            recovered_tail["result"]["envelopes"][0]["payload"]["operation"],
            "project.save"
        );
        let tail_ack = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                14,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": replacement_app,
                    "generation": replacement_generation,
                    "batch_id": tail_batch,
                    "operation": "project.save",
                    "outcome": "succeeded",
                }),
            )
        };
        assert_eq!(tail_ack["result"]["ack"]["acknowledged"], true);
        let recovered_refresh_tail = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                15,
                "transaction.receipt.pending",
                receipt_scope(&root, &replacement_app, replacement_generation),
            )
        };
        assert_eq!(
            recovered_refresh_tail["result"]["envelopes"][0]["batch_id"],
            refresh_batch
        );
        assert_eq!(
            recovered_refresh_tail["result"]["envelopes"][0]["payload"]["operation"],
            "project.external-refresh"
        );
        assert_eq!(
            recovered_refresh_tail["result"]["envelopes"][0]["status"],
            "pending"
        );
        let refresh_ack = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                16,
                "transaction.receipt.ack",
                json!({
                    "root": root,
                    "app_instance": replacement_app,
                    "generation": replacement_generation,
                    "batch_id": refresh_batch,
                    "operation": "project.external-refresh",
                    "outcome": "coalesced",
                }),
            )
        };
        assert_eq!(refresh_ack["result"]["ack"]["acknowledged"], true);
        let drained = {
            let replacement = &mut second_wave[second_winner_index];
            rpc(
                &mut replacement.1,
                &mut replacement.2,
                17,
                "transaction.receipt.pending",
                receipt_scope(&root, &replacement_app, replacement_generation),
            )
        };
        assert_eq!(drained["result"]["envelopes"], json!([]));
        assert!(!active_path.exists());
        assert_eq!(std::fs::read(&save_tmx).unwrap(), tmx_before);
        assert_eq!(
            std::fs::metadata(&save_tmx).unwrap().modified().unwrap(),
            tmx_mtime_before,
            "receipt recovery replayed the product TMX write at {point}"
        );
        let terminal_history = std::fs::read_to_string(&history_path).unwrap();
        for batch in [&terminal_batch, &receipt_batch, &tail_batch] {
            assert_eq!(
                terminal_history
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap())
                    .filter(|row| {
                        row["batch_id"] == batch.as_str() && row["status"] == "completed"
                    })
                    .count(),
                1,
                "product terminal history duplicated {batch} at {point}"
            );
        }
        assert_eq!(
            terminal_history
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter(|row| { row["batch_id"] == refresh_batch && row["status"] == "completed" })
                .count(),
            1,
            "refresh tail terminal history duplicated at {point}"
        );
        for replacement in &mut second_wave {
            replacement.0.kill().unwrap();
            replacement.0.wait().unwrap();
        }
    }
}

#[test]
fn unified_journal_migrates_refresh_envelopes_and_compacts_only_acked_work() {
    fn create_project(root: &std::path::Path, source: &str) {
        let props = omegat_core::properties::ProjectProperties::create(
            root.to_path_buf(),
            "en".into(),
            "fr".into(),
            false,
        );
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        std::fs::write(props.source_dir.join("source.txt"), source).unwrap();
    }

    fn spawn_sidecar(
        config: &std::path::Path,
        abort_compaction: Option<&str>,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        match abort_compaction {
            Some("archive") => {
                command.env("OMEGAT_TEST_ABORT_PRODUCT_COMPACTION_AFTER_ARCHIVE", "1");
            }
            Some("queue-rename") => {
                command.env(
                    "OMEGAT_TEST_ABORT_PRODUCT_COMPACTION_AFTER_QUEUE_RENAME",
                    "1",
                );
            }
            _ => {}
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    fn refresh_scope(root: &std::path::Path, app: &str, generation: u64) -> Value {
        json!({
            "root": root,
            "app_instance": app,
            "generation": generation,
        })
    }

    fn raw_journal(root: &std::path::Path, envelope_version: u8, unknown_payload: bool) -> Value {
        let mut payload = json!({
            "paths": [root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "terminal" },
            "sources": ["native"],
        });
        if unknown_payload {
            payload
                .as_object_mut()
                .unwrap()
                .insert("future_receipt_state".into(), json!("pending"));
        }
        json!({
            "version": 2,
            "project_root": root.canonicalize().unwrap(),
            "app_instance": "malformed-writer",
            "generation": 41,
            "batches": [{
                "version": envelope_version,
                "project_root": root.canonicalize().unwrap(),
                "generation": 41,
                "batch_id": "old-terminal-must-not-revive",
                "status": "completed",
                "error_code": null,
                "updated_unix_ms": 1,
                "payload": payload,
            }],
            "updated_unix_ms": 1,
        })
    }

    let temp = tempfile::tempdir().unwrap();
    let compact_root = temp.path().join("compact");
    let other_root = temp.path().join("other");
    let unknown_root = temp.path().join("unknown");
    let future_root = temp.path().join("future");
    for (root, source) in [
        (&compact_root, "compact source"),
        (&other_root, "other source"),
        (&unknown_root, "unknown source"),
        (&future_root, "future source"),
    ] {
        create_project(root, source);
    }

    let compact_config = temp.path().join("compact-config");
    let journal_path = compact_root.join(".repositories/transactions/active.json");
    let history_path = compact_root.join(".repositories/transactions/history.ndjson");
    let (mut first_child, mut first_in, mut first_out) = spawn_sidecar(&compact_config, None);
    rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.open",
        json!({ "root": compact_root }),
    );
    let first = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "project.refresh.enqueue",
        json!({
            "root": compact_root,
            "app_instance": "electron-before-compaction",
            "generation": 8,
            "paths": [compact_root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "unacked-receipt" },
            "sources": ["native"],
        }),
    );
    let receipt_batch = first["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    let refreshed = rpc(
        &mut first_in,
        &mut first_out,
        3,
        "project.external-refresh",
        json!({
            "transaction_project_root": compact_root,
            "transaction_generation": 8,
            "transaction_batch_id": receipt_batch,
            "app_instance": "electron-before-compaction",
        }),
    );
    assert_eq!(refreshed["error"], Value::Null);
    let second = rpc(
        &mut first_in,
        &mut first_out,
        4,
        "project.refresh.enqueue",
        json!({
            "root": compact_root,
            "app_instance": "electron-before-compaction",
            "generation": 8,
            "paths": [compact_root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "pending-tail" },
            "sources": ["sidecar"],
        }),
    );
    let pending_batch = second["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    first_child.kill().unwrap();
    first_child.wait().unwrap();

    let mut journal: Value =
        serde_json::from_slice(&std::fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(journal["batches"][0]["status"], "sidecar_committed");
    assert_eq!(journal["batches"][1]["status"], "pending");
    let receipt = journal["batches"][0]["commit"].clone();
    let mut acknowledged = journal["batches"][0].clone();
    acknowledged["batch_id"] = json!("acked-old-before-compaction");
    acknowledged["status"] = json!("completed");
    journal["batches"]
        .as_array_mut()
        .unwrap()
        .insert(0, acknowledged);
    std::fs::write(&journal_path, serde_json::to_vec_pretty(&journal).unwrap()).unwrap();

    let journal_before_interrupted_compaction = std::fs::read(&journal_path).unwrap();
    let (mut interrupted_child, mut interrupted_in, mut interrupted_out) =
        spawn_sidecar(&compact_config, Some("archive"));
    rpc(
        &mut interrupted_in,
        &mut interrupted_out,
        5,
        "project.open",
        json!({ "root": compact_root }),
    );
    writeln!(
        interrupted_in,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "transaction.receipt.pending",
            "params": refresh_scope(
                &compact_root,
                "electron-interrupted-compaction",
                1,
            ),
        })
    )
    .unwrap();
    interrupted_in.flush().unwrap();
    drop(interrupted_in);
    assert!(!interrupted_child.wait().unwrap().success());
    assert_eq!(
        std::fs::read(&journal_path).unwrap(),
        journal_before_interrupted_compaction,
        "interrupted compaction replaced the source journal"
    );
    let history_after_archive_fsync = std::fs::read(&history_path).unwrap();
    let archived_after_first_attempt: Vec<Value> =
        std::str::from_utf8(&history_after_archive_fsync)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .filter(|row: &Value| row["batch_id"] == "acked-old-before-compaction")
            .collect();
    assert_eq!(
        archived_after_first_attempt.len(),
        1,
        "archive-fsync interruption duplicated the terminal batch"
    );
    let after_interruption: Value =
        serde_json::from_slice(&journal_before_interrupted_compaction).unwrap();
    assert_eq!(after_interruption["batches"][0]["status"], "completed");
    assert_eq!(
        after_interruption["batches"][1]["status"],
        "sidecar_committed"
    );
    assert_eq!(after_interruption["batches"][1]["commit"], receipt);
    assert_eq!(after_interruption["batches"][2]["status"], "pending");

    let (mut renamed_child, mut renamed_in, mut renamed_out) =
        spawn_sidecar(&compact_config, Some("queue-rename"));
    rpc(
        &mut renamed_in,
        &mut renamed_out,
        51,
        "project.open",
        json!({ "root": compact_root }),
    );
    writeln!(
        renamed_in,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 61,
            "method": "transaction.receipt.pending",
            "params": refresh_scope(
                &compact_root,
                "electron-interrupted-queue-rename",
                1,
            ),
        })
    )
    .unwrap();
    renamed_in.flush().unwrap();
    drop(renamed_in);
    assert!(!renamed_child.wait().unwrap().success());
    let queue_after_rename: Value =
        serde_json::from_slice(&std::fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(queue_after_rename["batches"].as_array().unwrap().len(), 2);
    assert_eq!(
        queue_after_rename["batches"][0]["batch_id"],
        receipt_batch.as_str()
    );
    assert_eq!(
        queue_after_rename["batches"][0]["status"],
        "sidecar_committed"
    );
    assert_eq!(
        queue_after_rename["batches"][1]["batch_id"],
        pending_batch.as_str()
    );
    assert_eq!(queue_after_rename["batches"][1]["status"], "pending");
    assert_eq!(
        std::fs::read(&history_path).unwrap(),
        history_after_archive_fsync,
        "queue-rename recovery appended the already archived terminal batch again"
    );

    let (mut second_child, mut second_in, mut second_out) = spawn_sidecar(&compact_config, None);
    rpc(
        &mut second_in,
        &mut second_out,
        5,
        "project.open",
        json!({ "root": compact_root }),
    );
    let compacted = rpc(
        &mut second_in,
        &mut second_out,
        6,
        "transaction.receipt.pending",
        refresh_scope(&compact_root, "electron-after-compaction", 1),
    );
    assert_eq!(
        compacted["result"]["envelopes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|batch| batch["batch_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![receipt_batch.as_str()]
    );
    assert_eq!(
        compacted["result"]["envelopes"][0]["status"],
        "sidecar_committed"
    );
    assert_eq!(compacted["result"]["envelopes"][0]["commit"], receipt);
    assert!(compacted["result"]["envelopes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|batch| batch["generation"] == 1));
    let compacted_on_disk: Value =
        serde_json::from_slice(&std::fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(
        compacted_on_disk["batches"][0]["batch_id"],
        compacted["result"]["envelopes"][0]["batch_id"]
    );
    assert_eq!(
        compacted_on_disk["batches"][0]["status"],
        compacted["result"]["envelopes"][0]["status"]
    );
    assert_eq!(
        compacted_on_disk["batches"][0]["payload"]["refresh"],
        compacted["result"]["envelopes"][0]["payload"]
    );
    assert_eq!(compacted_on_disk["batches"][1]["batch_id"], pending_batch);
    assert_eq!(compacted_on_disk["batches"][1]["status"], "pending");
    assert_eq!(
        compacted_on_disk["batches"][1].get("commit"),
        None,
        "pending FIFO tail gained a receipt during compaction"
    );
    let history: Vec<Value> = std::fs::read_to_string(&history_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let old = history
        .iter()
        .filter(|row| row["batch_id"] == "acked-old-before-compaction")
        .collect::<Vec<_>>();
    assert_eq!(old.len(), 1);
    let old = old[0];
    assert_eq!(old["generation"], 8);
    assert_eq!(old["status"], "completed");

    let acknowledged_receipt = rpc(
        &mut second_in,
        &mut second_out,
        7,
        "transaction.receipt.ack",
        json!({
            "root": compact_root,
            "app_instance": "electron-after-compaction",
            "generation": 1,
            "batch_id": receipt_batch,
            "operation": "project.external-refresh",
            "outcome": "succeeded",
        }),
    );
    assert_eq!(acknowledged_receipt["result"]["ack"]["acknowledged"], true);
    let stale_generation = rpc(
        &mut second_in,
        &mut second_out,
        8,
        "transaction.receipt.pending",
        refresh_scope(&compact_root, "electron-after-compaction", 2),
    );
    assert_eq!(stale_generation["result"]["envelopes"], json!([]));

    let cross_project = rpc(
        &mut second_in,
        &mut second_out,
        9,
        "project.refresh.enqueue",
        json!({
            "root": compact_root,
            "app_instance": "electron-after-compaction",
            "generation": 2,
            "paths": [compact_root.join("source/source.txt")],
            "fingerprints": { "source/source.txt": "cross-project-stale" },
            "sources": ["native"],
        }),
    );
    let cross_project_batch = cross_project["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    rpc(
        &mut second_in,
        &mut second_out,
        10,
        "project.open",
        json!({ "root": other_root }),
    );
    let other_pending = rpc(
        &mut second_in,
        &mut second_out,
        11,
        "transaction.receipt.pending",
        refresh_scope(&other_root, "electron-after-compaction", 3),
    );
    assert_eq!(other_pending["result"]["envelopes"], json!([]));
    rpc(
        &mut second_in,
        &mut second_out,
        12,
        "project.open",
        json!({ "root": compact_root }),
    );
    let not_revived = rpc(
        &mut second_in,
        &mut second_out,
        13,
        "transaction.receipt.pending",
        refresh_scope(&compact_root, "electron-after-compaction", 4),
    );
    assert_eq!(not_revived["result"]["envelopes"], json!([]));
    let terminal_history = std::fs::read_to_string(&history_path).unwrap();
    assert!(terminal_history.lines().any(|line| {
        let row: Value = serde_json::from_str(line).unwrap();
        row["batch_id"] == cross_project_batch && row["status"] == "cancelled"
    }));
    second_child.kill().unwrap();
    second_child.wait().unwrap();

    for (root, config, envelope_version, unknown_payload, expected) in [
        (
            &unknown_root,
            temp.path().join("unknown-config"),
            1,
            true,
            "unknown field",
        ),
        (
            &future_root,
            temp.path().join("future-config"),
            2,
            false,
            "unsupported transaction envelope version 2",
        ),
    ] {
        let malformed_path = root.join(".repositories/transactions/external-refresh.json");
        std::fs::create_dir_all(malformed_path.parent().unwrap()).unwrap();
        let malformed =
            serde_json::to_vec_pretty(&raw_journal(root, envelope_version, unknown_payload))
                .unwrap();
        std::fs::write(&malformed_path, &malformed).unwrap();
        let (mut child, mut child_in, mut child_out) = spawn_sidecar(&config, None);
        rpc(
            &mut child_in,
            &mut child_out,
            20,
            "project.open",
            json!({ "root": root }),
        );
        let rejected = rpc(
            &mut child_in,
            &mut child_out,
            21,
            "transaction.receipt.pending",
            refresh_scope(root, "malformed-reader", 1),
        );
        assert_eq!(rejected["error"]["code"], -32603);
        assert!(
            rejected["error"]["message"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{rejected}"
        );
        assert_eq!(std::fs::read(&malformed_path).unwrap(), malformed);
        assert!(!root
            .join(".repositories/transactions/external-refresh-history.ndjson")
            .exists());
        child.kill().unwrap();
        child.wait().unwrap();
    }
}

#[test]
fn sidecar_commit_checkpoint_recovers_rebind_without_replaying_refresh() {
    fn spawn_sidecar(
        config: &std::path::Path,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
            .env("OMEGAT_CONFIG_DIR", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let root = temp.path().join("checkpoint-project");
    let props = omegat_core::properties::ProjectProperties::create(
        root.clone(),
        "en".into(),
        "fr".into(),
        false,
    );
    props.ensure_dirs().unwrap();
    props.write().unwrap();
    std::fs::write(props.source_dir.join("before.txt"), "before checkpoint").unwrap();

    let (mut first_child, mut first_in, mut first_out) = spawn_sidecar(&config);
    rpc(
        &mut first_in,
        &mut first_out,
        1,
        "project.open",
        json!({ "root": root }),
    );
    std::fs::write(
        props.source_dir.join("committed.txt"),
        "sidecar committed before renderer ack",
    )
    .unwrap();
    let enqueued = rpc(
        &mut first_in,
        &mut first_out,
        2,
        "project.refresh.enqueue",
        json!({
            "root": root,
            "app_instance": "electron-before-renderer-crash",
            "generation": 12,
            "paths": [props.source_dir.join("committed.txt")],
            "fingerprints": { "source/committed.txt": "checkpoint-1" },
            "sources": ["native"]
        }),
    );
    let batch_id = enqueued["result"]["batch"]["batch_id"]
        .as_str()
        .unwrap()
        .to_string();
    let refreshed = rpc(
        &mut first_in,
        &mut first_out,
        3,
        "project.external-refresh",
        json!({
            "progress_token": "checkpoint-refresh",
            "transaction_project_root": root,
            "transaction_generation": 12,
            "transaction_batch_id": batch_id,
            "app_instance": "electron-before-renderer-crash"
        }),
    );
    assert_eq!(refreshed["error"], Value::Null);
    assert_eq!(refreshed["result"]["entries"], 2);
    first_child.kill().unwrap();
    first_child.wait().unwrap();

    let (mut second_child, mut second_in, mut second_out) = spawn_sidecar(&config);
    rpc(
        &mut second_in,
        &mut second_out,
        4,
        "project.open",
        json!({ "root": root }),
    );
    let recovered = rpc(
        &mut second_in,
        &mut second_out,
        5,
        "transaction.receipt.pending",
        json!({
            "root": root,
            "app_instance": "electron-after-renderer-crash",
            "generation": 1
        }),
    );
    let checkpoint = &recovered["result"]["envelopes"][0];
    assert_eq!(checkpoint["batch_id"], batch_id);
    assert_eq!(checkpoint["status"], "sidecar_committed");
    assert_eq!(checkpoint["generation"], 1);
    assert_eq!(
        checkpoint["payload"]["committed_result"]["entry_list"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(checkpoint["commit"]["manifest_items"], 2);
    assert_eq!(
        checkpoint["commit"]["manifest_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );

    let entries = rpc(&mut second_in, &mut second_out, 6, "entry.list", json!({}));
    assert_eq!(entries["result"].as_array().unwrap().len(), 2);
    assert!(entries["result"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| { entry["source"] == "sidecar committed before renderer ack" }));

    let completed = rpc(
        &mut second_in,
        &mut second_out,
        7,
        "transaction.receipt.ack",
        json!({
            "root": root,
            "app_instance": "electron-after-renderer-crash",
            "generation": 1,
            "batch_id": batch_id,
            "operation": "project.external-refresh",
            "outcome": "succeeded"
        }),
    );
    assert_eq!(completed["result"]["ack"]["acknowledged"], true);
    let terminal: omegat_team::TransactionEnvelope<Value> = serde_json::from_str(
        std::fs::read_to_string(root.join(".repositories/transactions/history.ndjson"))
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(terminal.batch_id, batch_id);
    assert_eq!(terminal.generation, 1);
    assert_eq!(terminal.status, omegat_team::TransactionStatus::Completed);
    assert_eq!(terminal.error_code, None);
    assert_eq!(
        terminal.payload["refresh"]["operation"],
        "project.external-refresh"
    );
    assert!(!root.join(".repositories/transactions/active.json").exists());
    let _ = second_child.kill();
}

#[test]
fn refresh_product_result_and_checkpoint_share_one_fault_injected_publish() {
    fn spawn_sidecar(
        config: &std::path::Path,
        fault: Option<&str>,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"));
        command.env("OMEGAT_CONFIG_DIR", config);
        if let Some(fault) = fault {
            command.env("OMEGAT_TEST_ABORT_EXTERNAL_REFRESH_AT", fault);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        (child, stdin, stdout)
    }

    fn create_project(root: &std::path::Path, initial: &str) {
        let props = omegat_core::properties::ProjectProperties::create(
            root.to_path_buf(),
            "en".into(),
            "fr".into(),
            false,
        );
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        std::fs::write(props.source_dir.join("source.txt"), initial).unwrap();
    }

    fn start_faulted_refresh(
        config: &std::path::Path,
        root: &std::path::Path,
        app_instance: &str,
        fault: &str,
        replacement: &str,
    ) -> String {
        let (mut child, mut stdin, mut stdout) = spawn_sidecar(config, Some(fault));
        rpc(
            &mut stdin,
            &mut stdout,
            1,
            "project.open",
            json!({ "root": root }),
        );
        let source = root.join("source/source.txt");
        std::fs::write(&source, replacement).unwrap();
        let enqueued = rpc(
            &mut stdin,
            &mut stdout,
            2,
            "project.refresh.enqueue",
            json!({
                "root": root,
                "app_instance": app_instance,
                "generation": 9,
                "paths": [source],
                "fingerprints": { "source/source.txt": replacement },
                "sources": ["native"]
            }),
        );
        let batch_id = enqueued["result"]["batch"]["batch_id"]
            .as_str()
            .unwrap()
            .to_string();
        writeln!(
            stdin,
            "{}",
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "project.external-refresh",
                "params": {
                    "transaction_project_root": root,
                    "transaction_generation": 9,
                    "transaction_batch_id": batch_id,
                    "app_instance": app_instance
                }
            }))
            .unwrap()
        )
        .unwrap();
        stdin.flush().unwrap();
        drop(stdin);
        assert!(
            !child.wait().unwrap().success(),
            "{fault} did not terminate the sidecar"
        );
        batch_id
    }

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let before_root = temp.path().join("before-publish");
    let after_root = temp.path().join("after-publish");
    create_project(&before_root, "before");
    create_project(&after_root, "before");

    let before_batch = start_faulted_refresh(
        &config,
        &before_root,
        "electron-before-publish",
        "before_atomic_publish",
        "candidate rolled back before receipt",
    );
    let before_journal_path = before_root.join(".repositories/transactions/active.json");
    let before_journal: Value =
        serde_json::from_slice(&std::fs::read(&before_journal_path).unwrap()).unwrap();
    assert_eq!(before_journal["batches"][0]["status"], "pending");
    assert_eq!(
        before_journal["batches"][0]["payload"]["refresh"].get("committed_result"),
        None
    );
    assert_eq!(before_journal["batches"][0].get("commit"), None);

    let (mut replay_child, mut replay_in, mut replay_out) = spawn_sidecar(&config, None);
    rpc(
        &mut replay_in,
        &mut replay_out,
        4,
        "project.open",
        json!({ "root": before_root }),
    );
    let replay_pending = rpc(
        &mut replay_in,
        &mut replay_out,
        5,
        "transaction.receipt.pending",
        json!({
            "root": before_root,
            "app_instance": "electron-replay",
            "generation": 1
        }),
    );
    assert_eq!(
        replay_pending["result"]["envelopes"][0]["status"],
        "pending"
    );
    let replayed = rpc(
        &mut replay_in,
        &mut replay_out,
        6,
        "project.external-refresh",
        json!({
            "transaction_project_root": before_root,
            "transaction_generation": 1,
            "transaction_batch_id": before_batch,
            "app_instance": "electron-replay"
        }),
    );
    assert_eq!(replayed["error"], Value::Null);
    assert_eq!(replayed["result"]["entries"], 1);
    assert_eq!(
        replayed["result"]["entry_list"][0]["source"],
        "candidate rolled back before receipt"
    );
    let replay_committed: Value =
        serde_json::from_slice(&std::fs::read(&before_journal_path).unwrap()).unwrap();
    assert_eq!(
        replay_committed["batches"][0]["status"],
        "sidecar_committed"
    );
    assert_eq!(
        replay_committed["batches"][0]["commit"]["manifest_items"],
        1
    );
    rpc(
        &mut replay_in,
        &mut replay_out,
        7,
        "transaction.receipt.ack",
        json!({
            "root": before_root,
            "app_instance": "electron-replay",
            "generation": 1,
            "batch_id": before_batch,
            "operation": "project.external-refresh",
            "outcome": "succeeded"
        }),
    );
    let _ = replay_child.kill();

    let after_batch = start_faulted_refresh(
        &config,
        &after_root,
        "electron-after-publish",
        "after_atomic_publish",
        "committed exactly once before crash",
    );
    let after_journal_path = after_root.join(".repositories/transactions/active.json");
    let after_journal: Value =
        serde_json::from_slice(&std::fs::read(&after_journal_path).unwrap()).unwrap();
    assert_eq!(after_journal["batches"][0]["status"], "sidecar_committed");
    assert_eq!(after_journal["batches"][0]["commit"]["manifest_items"], 1);
    assert_eq!(
        after_journal["batches"][0]["payload"]["refresh"]["committed_result"]["entry_list"][0]
            ["source"],
        "committed exactly once before crash"
    );

    let (mut rebound_child, mut rebound_in, mut rebound_out) = spawn_sidecar(&config, None);
    rpc(
        &mut rebound_in,
        &mut rebound_out,
        8,
        "project.open",
        json!({ "root": after_root }),
    );
    let rebound_pending = rpc(
        &mut rebound_in,
        &mut rebound_out,
        9,
        "transaction.receipt.pending",
        json!({
            "root": after_root,
            "app_instance": "electron-rebind",
            "generation": 1
        }),
    );
    assert_eq!(
        rebound_pending["result"]["envelopes"][0]["status"],
        "sidecar_committed"
    );
    assert_eq!(
        rebound_pending["result"]["envelopes"][0]["payload"]["committed_result"]["entry_list"][0]
            ["source"],
        "committed exactly once before crash"
    );
    let rebound_entries = rpc(
        &mut rebound_in,
        &mut rebound_out,
        10,
        "entry.list",
        json!({}),
    );
    assert_eq!(
        rebound_entries["result"][0]["source"],
        "committed exactly once before crash"
    );
    rpc(
        &mut rebound_in,
        &mut rebound_out,
        11,
        "transaction.receipt.ack",
        json!({
            "root": after_root,
            "app_instance": "electron-rebind",
            "generation": 1,
            "batch_id": after_batch,
            "operation": "project.external-refresh",
            "outcome": "succeeded"
        }),
    );
    assert!(!after_journal_path.exists());
    let _ = rebound_child.kill();
}

#[test]
fn protocol_cancellation_preserves_existing_aligner_output() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.properties");
    let target = temp.path().join("target.properties");
    let dest = temp.path().join("aligned.tmx");
    let source_text = (0..5_000)
        .map(|index| format!("key{index}=source value {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let target_text = (0..5_000)
        .map(|index| format!("key{index}=target value {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&source, source_text).unwrap();
    std::fs::write(&target, target_text).unwrap();
    std::fs::write(&dest, b"preexisting aligned output").unwrap();

    let cancelled = cancel_at_checkpoint(
        &mut stdin,
        &mut stdout,
        1,
        "align.run",
        json!({
            "source": source,
            "target": target,
            "dest": dest,
            "mode": "parsewise",
            "segment": false,
            "source_lang": "en",
            "target_lang": "fr"
        }),
        "align.decode",
    );
    assert_eq!(
        cancelled["error"],
        json!({"code": -32800, "message": "request cancelled"})
    );
    assert_eq!(std::fs::read(&dest).unwrap(), b"preexisting aligned output");
    let siblings = file_snapshot(temp.path());
    assert!(siblings
        .iter()
        .all(|(path, _)| !path.contains(".omegat-align-")));
    let responsive = rpc(&mut stdin, &mut stdout, 2, "sys.version", json!({}));
    assert_eq!(responsive["result"]["version"], "6.2.0");
    let _ = child.kill();
}

#[test]
fn sidecar_self_writes_are_suppressed_before_real_external_changes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omegat-sidecar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sidecar");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("self-write-events");
    let _ = rpc(
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
    let source = root.join("source/chapter.txt");
    std::fs::write(&source, "Initial source").unwrap();
    let created_event = notification_for(&mut stdout, "project.files-changed");
    assert_eq!(created_event["params"]["paths"], json!([source]));
    let loaded = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
    assert_eq!(loaded["result"]["entries"], 1);
    let listed = rpc(&mut stdin, &mut stdout, 3, "entry.list", json!({}));
    let entry = &listed["result"][0];
    let _ = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "entry.set",
        json!({
            "index": 0,
            "key": entry["key"],
            "translation": "Traduction",
            "note": "",
            "revision": entry["revision"],
            "default_translation": true
        }),
    );
    let saved = rpc(&mut stdin, &mut stdout, 5, "project.save", json!({}));
    assert_eq!(saved["result"]["ok"], true);
    std::thread::sleep(Duration::from_millis(250));

    std::fs::write(&source, "Actual external source change").unwrap();
    let external = notification_for(&mut stdout, "project.files-changed");
    assert_eq!(
        external["params"],
        json!({
            "root": root.to_string_lossy(),
            "paths": [source.to_string_lossy()]
        })
    );
    let responsive = rpc(&mut stdin, &mut stdout, 6, "sys.version", json!({}));
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
    let _ = rpc(&mut stdin, &mut stdout, 2, "project.reload", json!({}));
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
    let entries = rpc(&mut stdin, &mut stdout, 4, "entry.list", json!({}));
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
