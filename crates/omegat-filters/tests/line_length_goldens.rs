//! assert_eq LineLengthLimitWriter Java goldens.

use omegat_filters::text::LineLengthLimitWriter;
use serde_json::Value;
use std::path::PathBuf;

fn golden(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/goldens")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn is_spaces_and_break_before_match_java() {
    let g = golden("engine/LineLengthLimitWriterTest#testIsSpaces.json");
    for c in g["cases"].as_array().unwrap() {
        let tok = c["token"].as_str().unwrap();
        assert_eq!(
            LineLengthLimitWriter::is_spaces_slice(&tok.chars().collect::<Vec<_>>()),
            c["spaces"].as_bool().unwrap(),
            "{tok}"
        );
    }
    let br = golden("engine/LineLengthLimitWriterTest#testIsPossibleBreakBefore.json");
    let text = br["text"].as_str().unwrap();
    for c in br["cases"].as_array().unwrap() {
        assert_eq!(
            LineLengthLimitWriter::is_possible_break_before_in(text, c["pos"].as_u64().unwrap() as usize),
            c["ok"].as_bool().unwrap(),
            "pos {}",
            c["pos"]
        );
    }
}

#[test]
fn outline_and_break_pos_match_java() {
    let out = golden("engine/LineLengthLimitWriterTest#testOutLine.json");
    assert_eq!(
        LineLengthLimitWriter::wrap(out["input"].as_str().unwrap(), 80, 100).trim_end(),
        out["output"].as_str().unwrap()
    );
    let empty = golden("engine/LineLengthLimitWriterTest#testOutLineWithEmptyBuffer.json");
    assert_eq!(LineLengthLimitWriter::wrap("", 80, 100).len() as u64, empty["length"].as_u64().unwrap());
    let none = golden("engine/LineLengthLimitWriterTest#testGetBreakPosNoBreakPossible.json");
    let input = none["input"].as_str().unwrap();
    assert_eq!(
        LineLengthLimitWriter::break_pos(input, 80, 100) as u64,
        none["break_pos"].as_u64().unwrap()
    );
    let beyond = golden("engine/LineLengthLimitWriterTest#testGetBreakPosBeyondMaxLength.json");
    let long = "This line contains more characters than allowed by max length restrictions";
    assert!(LineLengthLimitWriter::break_pos(long, 80, 100) as u64 <= beyond["max_length"].as_u64().unwrap());
}
