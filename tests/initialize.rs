mod common;

use common::LspTestHarness;

#[test]
fn test_initialize_returns_capabilities() {
    let mut harness = LspTestHarness::spawn();

    let response = harness.initialize().expect("Failed to get initialize response");

    assert!(response.contains("capabilities"), "Response should contain capabilities");
    assert!(response.contains("hoverProvider"), "Should support hover");
    assert!(response.contains("definitionProvider"), "Should support go-to-definition");
    assert!(response.contains("completionProvider"), "Should support completion");

    harness.shutdown();
}

#[test]
fn test_initialize_returns_server_info() {
    let mut harness = LspTestHarness::spawn();

    let response = harness.initialize().expect("Failed to get initialize response");

    assert!(response.contains("serverInfo"), "Response should contain serverInfo");
    assert!(response.contains("Lean"), "Server name should mention Lean");

    harness.shutdown();
}
