use std::path::Path;

use maximus_checks::{run_registered_checks, run_test_runner_config_check};
use maximus_core::{discover_project, Severity};

#[test]
fn test_runner_config_check_reports_jest_vitest_dual_config_and_registry_wiring() {
    let fixture = fixture("dual-config");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_test_runner_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("test-runner-dual-config:{}", fixture.to_string_lossy()),
        Severity::Warn,
        "Jest and Vitest configs coexist",
        "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.",
        "Pick one runner for this package, or document the split with separate config ownership and scripts.",
        Some(fixture.join("jest.config.js")),
    );

    let registered = run_registered_checks(&project).expect("registry should run");
    assert!(
        registered.findings.iter().any(|finding| {
            finding.id == format!("test-runner-dual-config:{}", fixture.to_string_lossy())
        }),
        "registry should include the test runner config check"
    );
}

#[test]
fn test_runner_config_check_accepts_single_runner_projects() {
    let fixture = fixture("vitest-only");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_test_runner_config_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "single-runner projects should not produce findings"
    );
}

#[test]
fn test_runner_config_check_detects_vitest_package_field_with_jest_file() {
    let fixture = fixture("package-field");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_test_runner_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("test-runner-dual-config:{}", fixture.to_string_lossy()),
        Severity::Warn,
        "Jest and Vitest configs coexist",
        "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.",
        "Pick one runner for this package, or document the split with separate config ownership and scripts.",
        Some(fixture.join("jest.config.js")),
    );
}

#[test]
fn test_runner_config_check_detects_package_json_dual_fields_without_config_files() {
    let fixture = fixture("package-only");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_test_runner_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("test-runner-dual-config:{}", fixture.to_string_lossy()),
        Severity::Warn,
        "Jest and Vitest configs coexist",
        "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.",
        "Pick one runner for this package, or document the split with separate config ownership and scripts.",
        Some(fixture.join("package.json")),
    );
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/fixtures/test-runners")
        .join(name)
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
    detail: &str,
    hint: &str,
    file: Option<std::path::PathBuf>,
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
    assert!(!finding.fixable);
    assert!(finding.fix_ids.is_empty());
}
