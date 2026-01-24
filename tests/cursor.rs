mod common;

use common::LspTestHarness;
use std::time::Duration;

#[test]
fn test_did_change_logs_cursor_position() {
    let mut harness = LspTestHarness::spawn();

    harness.initialize().expect("Failed to initialize");
    harness.initialized();

    // First, open the document
    let did_open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///home/wvhulle/Code/lean-tui/test.lean","languageId":"lean4","version":1,"text":"inductive MyNat where\n| zero: MyNat\n| succ: MyNat -> MyNat\n"}}}"#;
    harness.send(did_open).expect("Failed to send didOpen");

    std::thread::sleep(Duration::from_millis(500));

    // Send didChange notification (simulates typing in insert mode)
    let did_change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///home/wvhulle/Code/lean-tui/test.lean","version":2},"contentChanges":[{"range":{"start":{"line":3,"character":0},"end":{"line":3,"character":0}},"text":"x"}]}}"#;
    harness.send(did_change).expect("Failed to send didChange");

    std::thread::sleep(Duration::from_secs(1));

    let output = harness.collect_stderr();
    println!("stderr: {output}");

    assert!(
        output.contains("[lean-tui]"),
        "Should log cursor with [lean-tui] prefix"
    );
    assert!(output.contains("test.lean"), "Should contain file name");
    assert!(output.contains("3:0"), "Should contain edit position 3:0");
    assert!(
        output.contains("didChange"),
        "Should contain didChange method"
    );
}

#[test]
fn test_hover_logs_cursor_position() {
    let mut harness = LspTestHarness::spawn();

    harness.initialize().expect("Failed to initialize");
    harness.initialized();

    // Send hover request
    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///home/wvhulle/Code/lean-tui/test.lean"},"position":{"line":0,"character":5}}}"#;
    harness.send(hover).expect("Failed to send hover");

    std::thread::sleep(Duration::from_secs(2));

    let output = harness.collect_stderr();
    println!("stderr: {output}");

    assert!(
        output.contains("[lean-tui]"),
        "Should log cursor with [lean-tui] prefix"
    );
    assert!(output.contains("test.lean"), "Should contain file name");
    assert!(
        output.contains("0:5"),
        "Should contain line:character position"
    );
    assert!(
        output.contains("textDocument/hover"),
        "Should contain method name"
    );
}

#[test]
fn test_definition_logs_cursor_position() {
    let mut harness = LspTestHarness::spawn();

    harness.initialize().expect("Failed to initialize");
    harness.initialized();

    // Send definition request
    let definition = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///home/wvhulle/Code/lean-tui/test.lean"},"position":{"line":1,"character":3}}}"#;
    harness.send(definition).expect("Failed to send definition");

    std::thread::sleep(Duration::from_secs(2));

    let output = harness.collect_stderr();
    println!("stderr: {output}");

    assert!(
        output.contains("textDocument/definition"),
        "Should log definition request"
    );
    assert!(
        output.contains("1:3"),
        "Should contain line:character position"
    );
}

#[test]
fn test_completion_logs_cursor_position() {
    let mut harness = LspTestHarness::spawn();

    harness.initialize().expect("Failed to initialize");
    harness.initialized();

    // Send completion request
    let completion = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///home/wvhulle/Code/lean-tui/test.lean"},"position":{"line":2,"character":0}}}"#;
    harness.send(completion).expect("Failed to send completion");

    std::thread::sleep(Duration::from_secs(2));

    let output = harness.collect_stderr();
    println!("stderr: {output}");

    assert!(
        output.contains("textDocument/completion"),
        "Should log completion request"
    );
    assert!(
        output.contains("2:0"),
        "Should contain line:character position"
    );
}
