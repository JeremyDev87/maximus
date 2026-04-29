use std::fs;
use std::path::Path;

use maximus_checks::{
    run_node_matrix_check, run_registered_checks, run_registered_checks_with_config_root,
};
use maximus_core::{
    discover_project, discover_project_with_ignore_root, CheckFilterConfig, MaximusConfig, Severity,
};
use tempfile::TempDir;

#[test]
fn node_matrix_check_keeps_matching_engine_floor_and_actions_matrix_quiet() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [20, 22, 24]
            steps:
              - uses: actions/setup-node@v4
                with:
                  node-version: ${{ matrix.node }}
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "matrix that includes the engines.node floor should stay quiet"
    );
}

#[test]
fn node_matrix_check_parses_sequence_mapping_node_versions() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                include:
                  - node: 20
                  - node-version: 22
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "sequence mapping entries should contribute concrete Node matrix versions"
    );
}

#[test]
fn node_matrix_check_reports_missing_engine_floor_and_unsupported_matrix_version() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [18, 22]
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "node-matrix:{}",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "Node engine support and GitHub Actions matrix are out of sync",
        Some(fixture.path().join("package.json")),
    );
    let detail = &outcome.findings[0].detail;
    assert!(detail.contains("does not include supported Node 20"));
    assert!(detail.contains("unsupported Node 18"));
}

#[test]
fn node_matrix_check_reports_versions_above_inclusive_upper_bound() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20 <=22" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [20, 22, 24]
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "node-matrix:{}",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "Node engine support and GitHub Actions matrix are out of sync",
        Some(fixture.path().join("package.json")),
    );
    assert!(outcome.findings[0].detail.contains("unsupported Node 24"));
}

#[test]
fn node_matrix_check_reports_spaced_comparator_ranges() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">= 20 < 23" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [20, 22, 24]
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "node-matrix:{}",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "Node engine support and GitHub Actions matrix are out of sync",
        Some(fixture.path().join("package.json")),
    );
    assert!(outcome.findings[0].detail.contains("unsupported Node 24"));
}

#[test]
fn node_matrix_check_keeps_disjoint_comparator_ranges_quiet() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20 <21 || >=22 <23" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [20, 22]
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_node_matrix_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "matrix matching disjoint comparator ranges should stay quiet: {:?}",
        outcome.findings
    );
}

#[test]
fn registered_node_matrix_check_respects_ignored_workflow_files() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": ">=20" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/legacy.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node: [18]
        "#,
    );
    let config = MaximusConfig {
        checks: CheckFilterConfig {
            only: vec!["node-matrix".to_string()],
            ..CheckFilterConfig::default()
        },
        ignore_patterns: vec![".github/workflows/legacy.yml".to_string()],
        ..MaximusConfig::default()
    };
    let ignored_patterns = config.effective_ignore_patterns();
    let project =
        discover_project_with_ignore_root(fixture.path(), &ignored_patterns, fixture.path())
            .expect("project should discover");

    let outcome = run_registered_checks_with_config_root(&project, &config, fixture.path())
        .expect("registry should run");

    assert!(
        outcome.findings.is_empty(),
        "ignored workflow files should not affect node-matrix findings: {:?}",
        outcome.findings
    );
}

#[test]
fn registered_checks_include_node_matrix_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "engines": { "node": "^20 || ^22" } }"#,
    );
    write(
        fixture.path().join(".github/workflows/ci.yml"),
        r#"
        jobs:
          test:
            strategy:
              matrix:
                node:
                  - 20
                  - 24
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_registered_checks(&project).expect("registry should run");

    assert!(
        outcome.findings.iter().any(|finding| finding.id
            == format!(
                "node-matrix:{}",
                fixture.path().join("package.json").to_string_lossy()
            )),
        "registry should include node-matrix findings"
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
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
