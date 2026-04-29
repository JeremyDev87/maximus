use std::fs;
use std::path::Path;

use maximus_checks::{
    audit_project, run_ignore_file_drift_check, run_registered_checks_with_config_root,
};
use maximus_core::{
    discover_project, discover_project_with_ignore_root, CheckFilterConfig, MaximusConfig, Severity,
};
use tempfile::TempDir;

#[test]
fn ignore_file_drift_check_keeps_aligned_ignore_files_quiet() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join(".gitignore"),
        "node_modules\ndist/\n.tmp\n*.tgz\n",
    );
    write(
        fixture.path().join(".maximusignore"),
        "node_modules/\n/dist\ntmp\n*.tgz\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_ignore_file_drift_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "aligned artifact ignore patterns should stay quiet"
    );
}

#[test]
fn ignore_file_drift_check_keeps_escaped_artifact_literals_quiet() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(fixture.path().join(".gitignore"), "\\!dist\n\\#coverage\n");
    write(fixture.path().join(".maximusignore"), "");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_ignore_file_drift_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "escaped artifact-looking literals should not count as generated artifact ignores: {:?}",
        outcome.findings
    );
}

#[test]
fn ignore_file_drift_check_reports_generated_artifact_patterns_missing_from_gitignore() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join(".gitignore"),
        "node_modules\ncoverage\n",
    );
    write(
        fixture.path().join(".maximusignore"),
        "node_modules\ncoverage\ndist\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_ignore_file_drift_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "ignore-drift:{}:dist:.gitignore",
            fixture.path().to_string_lossy()
        ),
        Severity::Warn,
        "Ignore files disagree on generated artifact coverage",
        Some(fixture.path().join(".maximusignore")),
    );
}

#[test]
fn audit_project_runs_ignore_file_drift_check() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(fixture.path().join(".gitignore"), "node_modules\n");
    write(
        fixture.path().join(".maximusignore"),
        "node_modules\ntarget\n",
    );

    let audited = audit_project(fixture.path()).expect("audit should run");

    assert!(
        audited.result.findings.iter().any(|finding| finding.id
            == format!(
                "ignore-drift:{}:target:.gitignore",
                fixture.path().to_string_lossy()
            )),
        "registered audit path should include ignore-drift findings"
    );
}

#[test]
fn registered_ignore_file_drift_check_respects_config_ignore_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("fixtures/bad/.gitignore"),
        "node_modules\n",
    );
    write(
        fixture.path().join("fixtures/bad/.maximusignore"),
        "node_modules\ndist\n",
    );
    let config = MaximusConfig {
        checks: CheckFilterConfig {
            only: vec!["ignore-drift".to_string()],
            ..CheckFilterConfig::default()
        },
        ignore_patterns: vec!["fixtures/bad".to_string()],
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
        "ignored ignore-file directories should not produce findings: {:?}",
        outcome.findings
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
