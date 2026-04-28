use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn maximus_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

#[test]
fn fix_id_applies_only_the_selected_fix() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");
    write(
        root.join("packages/app/.env"),
        "API_URL=https://app.example\nAUTH_TOKEN=secretvalue123456\n",
    );

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--fix-id",
            &format!("env-example:create:{}", root.to_string_lossy()),
        ])
        .output()
        .expect("fix command should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(root.join(".env.example").is_file());
    assert!(!root.join("packages/app/.env.example").exists());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Applied: 1 fix(es)."));
    assert!(stdout.contains("Create .env.example"));
}

#[test]
fn fix_prefix_filters_dry_run_json_to_matching_fixes() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");
    write(
        root.join("packages/app/.env"),
        "API_URL=https://app.example\nAUTH_TOKEN=secretvalue123456\n",
    );
    write(root.join("packages/app/.env.example"), "API_URL=\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--fix-prefix",
            "env-example:sync:",
            "--json",
        ])
        .output()
        .expect("dry-run command should run");

    assert_eq!(output.status.code(), Some(1));

    let value: Value = serde_json::from_slice(&output.stdout).expect("json should parse");
    assert_eq!(value["initial"]["summary"]["fixesAvailable"], 1);
    assert_eq!(value["initial"]["fixes"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        value["initial"]["fixes"][0]["id"],
        format!(
            "env-example:sync:{}",
            root.join("packages/app").to_string_lossy()
        )
    );
}

#[test]
fn fix_command_errors_when_selector_matches_nothing() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--fix-id",
            "does-not-exist",
        ])
        .output()
        .expect("fix command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: No matching fixes for the requested selector.\n"
    );
}

#[test]
fn fix_id_requires_a_real_value_instead_of_another_flag() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--fix-id",
            "--dry-run",
        ])
        .output()
        .expect("fix command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Option \"--fix-id\" requires a value.\n"
    );
}

#[test]
fn fix_prefix_requires_a_real_value_instead_of_another_flag() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--fix-prefix",
            "--diff",
        ])
        .output()
        .expect("fix command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Option \"--fix-prefix\" requires a value.\n"
    );
}

#[test]
fn diff_requires_dry_run() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");

    let output = maximus_bin()
        .args(["fix", root.to_string_lossy().as_ref(), "--diff"])
        .output()
        .expect("fix command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Option \"--diff\" requires \"fix --dry-run\".\n"
    );
}

#[test]
fn fix_only_flags_are_rejected_without_fix_command() {
    let output = maximus_bin()
        .args(["--fix-id", "env-example:create:."])
        .output()
        .expect("command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Options \"--diff\", \"--env-source-comments\", \"--fix-id\", and \"--fix-prefix\" are only available for \"fix\".\n"
    );
}

#[test]
fn fix_only_flags_are_rejected_for_help_command() {
    let output = maximus_bin()
        .args(["help", "--fix-id", "env-example:create:."])
        .output()
        .expect("command should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be utf8")
        .contains("Usage\n  maximus audit [path]"));
}

#[test]
fn fix_only_flags_are_rejected_for_fix_help_command() {
    let output = maximus_bin()
        .args(["fix", "--help", "--fix-id", "env-example:create:."])
        .output()
        .expect("command should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be utf8")
        .contains("Usage\n  maximus audit [path]"));
}

#[test]
fn fix_only_flags_are_rejected_when_help_flag_precedes_fix_command() {
    let output = maximus_bin()
        .args(["--help", "fix", "--fix-id", "env-example:create:."])
        .output()
        .expect("command should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be utf8")
        .contains("Usage\n  maximus audit [path]"));
}

#[test]
fn audit_rejects_fix_only_flags() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");

    let output = maximus_bin()
        .args([
            "audit",
            root.to_string_lossy().as_ref(),
            "--fix-id",
            "env-example:",
        ])
        .output()
        .expect("audit command should run");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be utf8"),
        "Maximus failed: Options \"--diff\", \"--env-source-comments\", \"--fix-id\", and \"--fix-prefix\" are only available for \"fix\".\n"
    );
}

#[test]
fn diff_preview_shows_create_diff_without_writing_files() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(
        root.join(".env"),
        "API_URL=https://root.example\nAUTH_TOKEN=secretvalue123456\n",
    );

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--diff",
        ])
        .output()
        .expect("dry-run diff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(!root.join(".env.example").exists());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Preview diffs"));
    assert!(stdout.contains("--- /dev/null"));
    assert!(stdout.contains("+++ .env.example"));
    assert!(stdout.contains("+API_URL="));
    assert!(stdout.contains("+AUTH_TOKEN="));
}

#[test]
fn env_source_comments_group_fix_diff_without_changing_default_output() {
    let default_fixture = tempdir().expect("temp dir should exist");
    let default_root = default_fixture.path();
    let opt_in_fixture = tempdir().expect("temp dir should exist");
    let opt_in_root = opt_in_fixture.path();

    for root in [default_root, opt_in_root] {
        write(root.join(".env.local"), "LOCAL_Z=1\nLOCAL_A=2\n");
        write(root.join(".env"), "BASE_Z=1\nBASE_A=2\n");
    }

    let default_output = maximus_bin()
        .args(["fix", default_root.to_string_lossy().as_ref()])
        .output()
        .expect("default fix command should run");
    assert_eq!(default_output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(default_root.join(".env.example"))
            .expect("default example should write"),
        "BASE_A=\nBASE_Z=\nLOCAL_A=\nLOCAL_Z=\n"
    );

    let opt_in_output = maximus_bin()
        .args([
            "fix",
            opt_in_root.to_string_lossy().as_ref(),
            "--dry-run",
            "--diff",
            "--env-source-comments",
        ])
        .output()
        .expect("opt-in dry-run diff should run");

    assert_eq!(opt_in_output.status.code(), Some(1));
    assert!(!opt_in_root.join(".env.example").exists());
    let stdout = String::from_utf8(opt_in_output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("+# Source: .env"));
    assert!(stdout.contains("+BASE_A="));
    assert!(stdout.contains("+# Source: .env.local"));
    assert!(stdout.contains("+LOCAL_A="));
}

#[test]
fn env_source_comments_include_gitignored_env_inputs() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    fs::create_dir_all(root.join(".git")).expect("git dir should exist");
    write(root.join(".gitignore"), ".env\n.env.local\n");
    write(root.join(".env.local"), "LOCAL_Z=1\nLOCAL_A=2\n");
    write(root.join(".env"), "BASE_Z=1\nBASE_A=2\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--diff",
            "--env-source-comments",
        ])
        .output()
        .expect("opt-in dry-run diff should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(!root.join(".env.example").exists());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("+# Source: .env"));
    assert!(stdout.contains("+BASE_A="));
    assert!(stdout.contains("+# Source: .env.local"));
    assert!(stdout.contains("+LOCAL_A="));
}

#[test]
fn diff_preview_shows_update_diff_without_mutating_existing_file() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(
        root.join(".env"),
        "API_URL=https://root.example\nAUTH_TOKEN=secretvalue123456\n",
    );
    write(root.join(".env.example"), "API_URL=\n");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--diff",
        ])
        .output()
        .expect("dry-run diff should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(root.join(".env.example")).expect("file should remain unchanged"),
        "API_URL=\n"
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("+++ .env.example"));
    assert!(stdout.contains("@@ -1,1 +1,2 @@"));
    assert!(stdout.contains(" API_URL="));
    assert!(stdout.contains("+AUTH_TOKEN="));
}

#[test]
fn diff_preview_treats_existing_empty_env_example_as_update() {
    let fixture = tempdir().expect("temp dir should exist");
    let root = fixture.path();

    write(root.join(".env"), "API_URL=https://root.example\n");
    write(root.join(".env.example"), "");

    let output = maximus_bin()
        .args([
            "fix",
            root.to_string_lossy().as_ref(),
            "--dry-run",
            "--diff",
        ])
        .output()
        .expect("dry-run diff should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(root.join(".env.example")).expect("file should remain unchanged"),
        ""
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Append missing keys to .env.example"));
    assert!(stdout.contains("--- .env.example"));
    assert!(stdout.contains("+++ .env.example"));
    assert!(stdout.contains("@@ -0,0 +1,1 @@"));
    assert!(!stdout.contains("/dev/null"));
}

fn write(path: PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, contents).expect("file should write");
}
