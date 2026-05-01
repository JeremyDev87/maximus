use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn maximus_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

#[test]
fn audit_output_file_writes_report_creates_parents_and_keeps_exit_code() {
    let fixture = tempdir().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    let output_path = fixture.path().join("reports/nested/audit.json");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--json",
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");

    let value: Value =
        serde_json::from_str(&fs::read_to_string(&output_path).expect("output file should exist"))
            .expect("output file should contain valid json");
    assert_eq!(value["generator"], "maximus");
    assert!(
        value["findings"]
            .as_array()
            .is_some_and(|findings| !findings.is_empty()),
        "expected audit findings in file output: {value:?}"
    );
}

#[test]
fn output_dash_preserves_stdout_behavior() {
    let fixture = tempdir().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());

    let default_output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");
    let dash_output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--json",
            "--output",
            "-",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(dash_output.status.code(), default_output.status.code());
    assert_eq!(dash_output.stdout, default_output.stdout);
    assert_eq!(dash_output.stderr, default_output.stderr);
}

#[test]
fn fix_dry_run_output_file_writes_selected_report_without_stdout() {
    let fixture = tempdir().expect("temp dir should exist");
    write(
        fixture.path().join(".env"),
        "API_URL=https://example.test\n",
    );
    let output_path = fixture.path().join("reports/fix.json");

    let output = maximus_bin()
        .args([
            "fix",
            fixture.path().to_string_lossy().as_ref(),
            "--dry-run",
            "--json",
            "--output",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("fix should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");

    let value: Value =
        serde_json::from_str(&fs::read_to_string(&output_path).expect("output file should exist"))
            .expect("output file should contain valid json");
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["initial"]["generator"], "maximus");
    assert_eq!(value["applied"].as_array().map(Vec::len), Some(0));
}

#[test]
fn fix_output_file_write_error_fails_before_applying_mutations() {
    let fixture = tempdir().expect("temp dir should exist");
    write(
        fixture.path().join(".env"),
        "API_URL=https://example.test\n",
    );

    let output = maximus_bin()
        .args([
            "fix",
            fixture.path().to_string_lossy().as_ref(),
            "--json",
            "--output",
            fixture.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("fix should run");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Maximus 실패:"),
        "{output:?}"
    );
    assert!(
        !fixture.path().join(".env.example").exists(),
        "fix should not mutate target files after output preflight fails"
    );
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
