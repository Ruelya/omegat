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
    "script.run",
    "align.run",
    "align.edit",
    "aligner.configure",
    "wiki.import",
    "med.open",
    "project.convert",
    "project.import",
    "script.slot",
];

fn rpc(child_in: &mut impl Write, child_out: &mut impl BufRead, id: i64, method: &str, params: Value) -> Value {
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
