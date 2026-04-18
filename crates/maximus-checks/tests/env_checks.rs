use std::fs;
use std::path::{Path, PathBuf};

#[path = "../src/env.rs"]
mod env;

use env::{render_created_env_example, render_synced_env_example, run_env_check};
use maximus_core::{discover_project, FixPlan, Severity};
use tempfile::TempDir;

#[test]
fn env_check_matches_js_findings_for_duplicates_invalid_sync_secret_override_and_missing_concrete() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join(".env"),
        "PRIMARY=one\nDUP=first\nexport DUP=second\nNOT VALID\nSHARED=base\nONLY_BASE=1\n",
    );
    write(
        fixture.path().join(".env.local"),
        "SHARED=local\nLOCAL_ONLY=enabled\n",
    );
    write(
        fixture.path().join(".env.example"),
        "PRIMARY=\nDUP=\nSHARED=sk_live_1234567890abcdef\nCI_ONLY=\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "env-duplicate:{}:DUP:3",
            fixture.path().join(".env").to_string_lossy()
        ),
        Severity::Error,
        "Duplicate env key \"DUP\"",
        "DUP is declared on lines 2 and 3.",
        "Keep one declaration per env file so overrides stay explicit.",
        Some(fixture.path().join(".env")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-invalid:{}:4", fixture.path().join(".env").to_string_lossy()),
        Severity::Warn,
        "Invalid env syntax",
        "Line 4 could not be parsed as KEY=value.",
        "Use shell-style env syntax or move comments to their own line.",
        Some(fixture.path().join(".env")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-example-sync:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        ".env.example is missing keys",
        "Missing keys: ONLY_BASE, LOCAL_ONLY.",
        "Run \"maximus fix\" to append the missing keys to .env.example.",
        Some(fixture.path().join(".env.example")),
        true,
        &[format!("env-example:sync:{}", fixture.path().to_string_lossy())],
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "env-example-secret:{}:SHARED",
            fixture.path().join(".env.example").to_string_lossy()
        ),
        Severity::Warn,
        ".env.example appears to contain a real value for \"SHARED\"",
        "Contract files should describe the interface, not ship concrete secrets.",
        "Replace the value with a blank or placeholder string before sharing the repo.",
        Some(fixture.path().join(".env.example")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-mismatch:{}", fixture.path().to_string_lossy()),
        Severity::Info,
        "Local env overrides detected",
        ".env.local overrides 1 key(s): SHARED.",
        "Make sure local-only overrides are intentional and documented in .env.example.",
        Some(fixture.path().join(".env.local")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-missing-concrete:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Declared env contract is not satisfied locally",
        "No concrete value was found for: CI_ONLY.",
        "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files.",
        Some(fixture.path().join(".env.example")),
        false,
        &[],
    );

    assert_has_fix(
        &outcome.fixes,
        &format!("env-example:sync:{}", fixture.path().to_string_lossy()),
        "Append missing keys to .env.example",
        &[fixture.path().join(".env.example")],
    );
}

#[test]
fn env_check_plans_example_creation_when_runtime_env_files_exist_without_contract() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("apps/web/.env.local"),
        "BETA_FLAG=1\nAPI_TOKEN=abcdef1234567890\n",
    );
    write(
        fixture.path().join("apps/web/.env.production"),
        "API_TOKEN=abcdef1234567890\nPUBLIC_URL=https://example.test\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");
    let dir = fixture.path().join("apps/web");

    assert_has_finding(
        &outcome.findings,
        &format!("env-example-missing:{}", dir.to_string_lossy()),
        Severity::Warn,
        "Missing .env.example contract",
        "Runtime env files exist, but .env.example is missing.",
        "Run \"maximus fix\" to create a blank contract file.",
        Some(dir.join(".env.local")),
        true,
        &[format!("env-example:create:{}", dir.to_string_lossy())],
    );
    assert_has_fix(
        &outcome.fixes,
        &format!("env-example:create:{}", dir.to_string_lossy()),
        "Create apps/web/.env.example",
        &[dir.join(".env.example")],
    );
}

#[test]
fn env_example_render_helpers_match_js_create_and_sync_semantics() {
    assert_eq!(
        render_created_env_example(["ZETA", "ALPHA", "ALPHA"]),
        "ALPHA=\nZETA=\n"
    );

    let synced = render_synced_env_example(
        "PRIMARY=\n",
        &["ZETA".to_string(), "ALPHA".to_string()],
    );
    assert_eq!(synced, "PRIMARY=\nALPHA=\nZETA=\n");

    let synced_without_trailing_newline = render_synced_env_example(
        "PRIMARY=",
        &["ZETA".to_string(), "ALPHA".to_string()],
    );
    assert_eq!(synced_without_trailing_newline, "PRIMARY=\nALPHA=\nZETA=\n");

    let synced_with_js_like_locale_order = render_synced_env_example(
        "PRIMARY=\n",
        &[
            "API_URL".to_string(),
            "API-URL".to_string(),
            "API.URL".to_string(),
        ],
    );
    assert_eq!(
        synced_with_js_like_locale_order,
        "PRIMARY=\nAPI_URL=\nAPI-URL=\nAPI.URL=\n"
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn assert_has_fix(fixes: &[FixPlan], id: &str, title: &str, files: &[PathBuf]) {
    let fix = fixes
        .iter()
        .find(|fix| fix.id == id)
        .unwrap_or_else(|| panic!("missing fix {id}"));

    assert_eq!(fix.title, title);
    assert_eq!(fix.files, files);
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
    detail: &str,
    hint: &str,
    file: Option<PathBuf>,
    fixable: bool,
    fix_ids: &[String],
) {
    let finding = findings
        .iter()
        .find(|finding| finding.id == id)
        .unwrap_or_else(|| panic!("missing finding {id}"));

    assert_eq!(finding.severity, severity);
    assert_eq!(finding.title, title);
    assert_eq!(finding.detail, detail);
    assert_eq!(finding.hint, hint);
    assert_eq!(finding.file, file);
    assert_eq!(finding.fixable, fixable);
    assert_eq!(finding.fix_ids, fix_ids);
}
