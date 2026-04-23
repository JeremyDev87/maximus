use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn maximus_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

#[test]
fn default_fail_on_applies_to_warning_findings_across_commands() {
    let fixture = warning_fixture();
    let target = fixture.path().to_string_lossy().into_owned();

    let audit_output = maximus_bin()
        .args(["audit", target.as_str(), "--json"])
        .output()
        .expect("audit should run");
    assert_eq!(audit_output.status.code(), Some(1), "{audit_output:?}");
    assert_finding_severity(&audit_output, "warn");

    for args in [
        vec!["doctor", target.as_str()],
        vec!["fix", target.as_str(), "--dry-run"],
    ] {
        let output = maximus_bin().args(args).output().expect("command should run");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
    }
}

#[test]
fn explicit_info_escalates_info_only_findings() {
    let fixture = info_only_fixture();
    let target = fixture.path().to_string_lossy().into_owned();

    for args in [
        vec!["audit", target.as_str(), "--fail-on", "info", "--json"],
        vec!["doctor", target.as_str(), "--fail-on", "info"],
        vec!["fix", target.as_str(), "--dry-run", "--fail-on", "info"],
    ] {
        let output = maximus_bin().args(args).output().expect("command should run");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
    }
}

#[test]
fn explicit_none_suppresses_warning_only_findings() {
    let fixture = warning_fixture();
    let target = fixture.path().to_string_lossy().into_owned();

    for args in [
        vec!["audit", target.as_str(), "--fail-on", "none", "--json"],
        vec!["doctor", target.as_str(), "--fail-on", "none"],
        vec!["fix", target.as_str(), "--dry-run", "--fail-on", "none"],
    ] {
        let output = maximus_bin().args(args).output().expect("command should run");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
}

fn info_only_fixture() -> TempDir {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_env_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"
        {
          "checks": { "only": ["env"] },
          "severity": { "env-mismatch": "info" }
        }
        "#,
    );

    fixture
}

fn warning_fixture() -> TempDir {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    fixture
}

fn write_env_fixture(root: &Path) {
    write(root.join(".env"), "SHARED=base\n");
    write(root.join(".env.local"), "SHARED=local\n");
    write(root.join(".env.example"), "SHARED=\n");
}

fn write_tsconfig_conflict_fixture(root: &Path) {
    write(
        root.join("package.json"),
        r##"{"name":"fixture","imports":{"#app/*":"./src/runtime/*"}}"##,
    );
    write(
        root.join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "#app/*": ["./src/lib/*"]
            }
          }
        }
        "##,
    );
    write(root.join("src/runtime/index.ts"), "export {};\n");
    write(root.join("src/lib/index.ts"), "export {};\n");
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn assert_finding_severity(output: &Output, expected_severity: &str) {
    let value: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0]
            .as_object()
            .and_then(|finding| finding.get("severity"))
            .and_then(Value::as_str),
        Some(expected_severity)
    );
}
