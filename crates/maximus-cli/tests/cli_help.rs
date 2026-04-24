use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn maximus_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

#[test]
fn no_args_prints_help() {
    let output = maximus_bin().output().expect("help command should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be utf8"),
        [
            "Maximus",
            "",
            "Bring order to chaotic configs.",
            "",
            "Usage",
            "  maximus audit [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json]",
            "  maximus doctor [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json]",
            "  maximus fix [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--dry-run] [--diff] [--fix-id <id>] [--fix-prefix <prefix>] [--format <format>] [--json]",
            "  maximus help",
            "",
        ]
        .join("\n")
    );
}

#[test]
fn help_subcommand_prints_usage() {
    let output = maximus_bin()
        .arg("help")
        .output()
        .expect("help subcommand should run");

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be utf8")
        .contains(
            "maximus audit [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json]"
        ));
}

#[test]
fn audit_json_routes_to_clean_skeleton_result() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args(["audit", fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert!(output.status.success());

    let value: Value =
        serde_json::from_slice(&output.stdout).expect("audit json output should be valid");
    assert_eq!(value["rootDir"], fixture.to_string_lossy().to_string());
    assert_eq!(value["summary"]["status"], "clean");
    assert_eq!(value["summary"]["blockingFindings"], 0);
    assert_eq!(value["structure"]["configFiles"], 1);
    assert_eq!(value["findings"], Value::Array(Vec::new()));
    assert_eq!(value["fixes"], Value::Array(Vec::new()));
}

#[test]
fn audit_format_markdown_routes_to_markdown_report() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args([
            "audit",
            fixture.to_string_lossy().as_ref(),
            "--format",
            "markdown",
        ])
        .output()
        .expect("audit markdown should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.starts_with("# Maximus audit\n"));
    assert!(stdout.contains("- Status: `clean`"));
}

#[test]
fn audit_format_sarif_routes_to_sarif_report() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args([
            "audit",
            fixture.to_string_lossy().as_ref(),
            "--format",
            "sarif",
        ])
        .output()
        .expect("audit sarif should run");

    assert!(output.status.success());
    let value: Value =
        serde_json::from_slice(&output.stdout).expect("audit sarif output should be valid");
    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["properties"]["reportKind"], "audit");
}

#[test]
fn doctor_format_sarif_routes_to_doctor_sarif_report() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args([
            "doctor",
            fixture.to_string_lossy().as_ref(),
            "--format",
            "sarif",
        ])
        .output()
        .expect("doctor sarif should run");

    assert!(output.status.success());
    let value: Value =
        serde_json::from_slice(&output.stdout).expect("doctor sarif output should be valid");
    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["properties"]["reportKind"], "doctor");
}

#[test]
fn audit_reference_env_matches_js_cli_output_and_status() {
    let fixture = fixture_path_for("reference-env");

    let rust_output = maximus_bin()
        .args(["audit", fixture.to_string_lossy().as_ref()])
        .output()
        .expect("rust audit should run");
    let js_output = js_maximus()
        .args(["audit", fixture.to_string_lossy().as_ref()])
        .output()
        .expect("js audit should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());
    assert_eq!(rust_output.stdout, js_output.stdout);
    assert_eq!(rust_output.stderr, js_output.stderr);
}

#[test]
fn audit_reference_tsconfig_matches_js_cli_output_and_status() {
    let fixture = fixture_path_for("reference-tsconfig");

    let rust_output = maximus_bin()
        .args(["audit", fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("rust audit should run");
    let js_output = js_maximus()
        .args(["audit", fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("js audit should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());
    assert_eq!(rust_output.stdout, js_output.stdout);
    assert_eq!(rust_output.stderr, js_output.stderr);
}

#[test]
fn fix_reference_env_dry_run_matches_js_cli_output_and_status() {
    let fixture = fixture_path_for("reference-env");

    let rust_output = maximus_bin()
        .args(["fix", fixture.to_string_lossy().as_ref(), "--dry-run"])
        .output()
        .expect("rust fix dry-run should run");
    let js_output = js_maximus()
        .args(["fix", fixture.to_string_lossy().as_ref(), "--dry-run"])
        .output()
        .expect("js fix dry-run should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());
    assert_eq!(rust_output.stdout, js_output.stdout);
    assert_eq!(rust_output.stderr, js_output.stderr);
}

#[test]
fn fix_reference_env_json_matches_js_cli_output_and_status() {
    let rust_fixture = prepare_temp_reference_env();
    let js_fixture = prepare_temp_reference_env();

    let rust_output = maximus_bin()
        .args(["fix", rust_fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("rust fix json should run");
    let js_output = js_maximus()
        .args(["fix", js_fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("js fix json should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());

    let rust_json: Value =
        serde_json::from_slice(&rust_output.stdout).expect("rust json should parse");
    let js_json: Value = serde_json::from_slice(&js_output.stdout).expect("js json should parse");

    assert_eq!(rust_json["dryRun"], js_json["dryRun"]);
    assert_eq!(rust_json["applied"][0]["outcome"], "created");
    assert_eq!(
        rust_json["applied"][0]["outcome"],
        js_json["applied"][0]["outcome"]
    );
    assert_eq!(
        rust_json["initial"]["summary"]["status"],
        js_json["initial"]["summary"]["status"]
    );
    assert_eq!(
        rust_json["final"]["summary"]["status"],
        js_json["final"]["summary"]["status"]
    );
}

#[test]
fn doctor_text_uses_expected_sections() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args(["doctor", fixture.to_string_lossy().as_ref()])
        .output()
        .expect("doctor should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be utf8"),
        [
            "Maximus doctor",
            &format!("Target: {}", fixture.to_string_lossy()),
            "",
            "Diagnosis: clean",
            "Project shape: single package, 1 package(s), 1 config file(s), 0 env folder(s)",
            "",
            "Prescription",
            "- No automatic fixes are currently available.",
            "- No manual follow-up is required right now.",
            "",
            "No config drift detected.",
            "",
            "Recommended structure",
            "- Current config surface looks healthy. Keep shared rules centralized as the repo grows.",
            "",
        ]
        .join("\n")
    );
}

#[test]
fn fix_dry_run_json_keeps_js_top_level_contract() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args([
            "fix",
            fixture.to_string_lossy().as_ref(),
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("fix dry-run should run");

    assert!(output.status.success());

    let value: Value =
        serde_json::from_slice(&output.stdout).expect("fix json output should be valid");
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["targetDir"], fixture.to_string_lossy().to_string());
    assert!(value.get("initial").is_some());
    assert!(value.get("applied").is_some());
    assert!(value.get("final").is_some());
}

#[test]
fn fix_format_sarif_fails_closed() {
    let fixture = fixture_path();
    let output = maximus_bin()
        .args([
            "fix",
            fixture.to_string_lossy().as_ref(),
            "--dry-run",
            "--format",
            "sarif",
        ])
        .output()
        .expect("fix sarif rejection should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Option \"--format sarif\" is only available for \"audit\" and \"doctor\".\n"
    );
}

#[test]
fn unknown_command_uses_prefixed_stderr_and_exit_code_two() {
    let output = maximus_bin()
        .arg("foobar")
        .output()
        .expect("unknown command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Unknown command \"foobar\". Run \"maximus help\" for usage.\n"
    );
}

#[test]
fn unknown_command_does_not_load_broken_config() {
    let fixture = tempdir().expect("fixture should exist");
    let config_path = fixture.path().join("maximus.config.json");
    fs::write(&config_path, r#"{ "checks": { "only": ["env",] }"#)
        .expect("broken config should write");

    let output = maximus_bin()
        .args(["foobar", fixture.path().to_string_lossy().as_ref()])
        .output()
        .expect("unknown command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Unknown command \"foobar\". Run \"maximus help\" for usage.\n"
    );
}

#[test]
fn missing_directory_uses_prefixed_stderr_and_exit_code_two() {
    let output = maximus_bin()
        .args(["audit", "/definitely/not-a-real-path"])
        .output()
        .expect("missing directory case should run");

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.starts_with("Maximus failed: "));
}

#[cfg(unix)]
#[test]
fn non_utf8_path_argument_does_not_panic_before_delegation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let non_utf8_path = PathBuf::from(OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', b'f', b'o', 0x80, b'o',
    ]));

    let output = maximus_bin()
        .arg("audit")
        .arg(&non_utf8_path)
        .output()
        .expect("rust audit should run");

    assert!(output.status.code().is_some());
}

fn fixture_path() -> PathBuf {
    workspace_root().join("test/fixtures/clean-project")
}

fn fixture_path_for(name: &str) -> PathBuf {
    workspace_root().join("test/fixtures").join(name)
}

fn prepare_temp_reference_env() -> PathBuf {
    let temp = tempdir().expect("temp dir should exist");
    let target = temp.keep();
    let source = fixture_path_for("reference-env").join(".env");

    fs::create_dir_all(&target).expect("target dir should exist");
    fs::copy(source, target.join(".env")).expect("env fixture should copy");

    target
}

fn js_maximus() -> Command {
    let mut command = Command::new("node");
    command.arg(workspace_root().join("bin/maximus.js"));
    command
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}
