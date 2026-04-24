use std::path::Path;

use maximus_checks::{run_registered_checks, run_workspace_config_check};
use maximus_core::{discover_project, Severity};

#[test]
fn workspace_config_check_reports_empty_workspace_files_and_registry_wiring() {
    let fixture = fixture("empty");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_workspace_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "workspace-config:{}",
            fixture.join("pnpm-workspace.yaml").to_string_lossy()
        ),
        "pnpm-workspace.yaml does not declare any package patterns",
        Severity::Warn,
        Some(fixture.join("pnpm-workspace.yaml")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "workspace-config:{}",
            fixture.join("turbo.json").to_string_lossy()
        ),
        "turbo.json does not declare any workspace tasks",
        Severity::Warn,
        Some(fixture.join("turbo.json")),
    );

    let registered = run_registered_checks(&project).expect("registry should run");
    assert!(
        registered.findings.iter().any(|finding| {
            finding.id
                == format!(
                    "workspace-config:{}",
                    fixture.join("pnpm-workspace.yaml").to_string_lossy()
                )
        }),
        "registry should include the workspace config check"
    );
}

#[test]
fn workspace_config_check_reports_parse_errors_and_accepts_valid_workspace_files() {
    let parse_error = fixture("parse-error");
    let valid = fixture("valid");
    let inline_array = fixture("inline-array");

    let parse_project = discover_project(&parse_error).expect("project should discover");
    let parse_outcome = run_workspace_config_check(&parse_project).expect("check should run");
    assert_has_finding(
        &parse_outcome.findings,
        &format!(
            "workspace-config:{}",
            parse_error.join("turbo.json").to_string_lossy()
        ),
        "turbo.json could not be parsed",
        Severity::Warn,
        Some(parse_error.join("turbo.json")),
    );

    let valid_project = discover_project(&valid).expect("project should discover");
    let valid_outcome = run_workspace_config_check(&valid_project).expect("check should run");
    assert!(
        valid_outcome.findings.is_empty(),
        "valid workspace files should not produce findings"
    );

    let inline_project = discover_project(&inline_array).expect("project should discover");
    let inline_outcome = run_workspace_config_check(&inline_project).expect("check should run");
    assert!(
        inline_outcome.findings.is_empty(),
        "valid inline array package globs should not produce findings"
    );
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/fixtures/workspace-config")
        .join(name)
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    title: &str,
    severity: Severity,
    file: Option<std::path::PathBuf>,
) {
    let finding = findings
        .iter()
        .find(|finding| finding.id == id)
        .unwrap_or_else(|| panic!("missing finding {id}"));

    assert_eq!(finding.severity, severity);
    assert_eq!(finding.title, title);
    assert_eq!(finding.file, file);
}
