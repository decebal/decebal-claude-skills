use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn valid_listing_passes_binary_lint() {
    let output = Command::new(env!("CARGO_BIN_EXE_aso-lint"))
        .args(["lint", "--input", fixture("apple.json").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["platform"], "apple");
}

#[test]
fn invalid_listing_returns_findings_exit() {
    let output = Command::new(env!("CARGO_BIN_EXE_aso-lint"))
        .args([
            "lint",
            "--input",
            fixture("google-invalid.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["checks"][0]["id"], "ASO-GOOGLE-NAME");
}
