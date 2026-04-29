use std::fs;
use std::path::Path;

use maximus_checks::{audit_project, audit_project_with_config, run_npmignore_files_check};
use maximus_core::{discover_project, MaximusConfig, Severity};
use tempfile::TempDir;

#[test]
fn npmignore_files_check_keeps_non_overlapping_publish_surface_quiet() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist", "README.md"] }"#,
    );
    write(fixture.path().join(".npmignore"), "coverage\n*.log\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "non-overlapping package files and .npmignore should stay quiet"
    );
}

#[test]
fn npmignore_files_check_ignores_root_npmignore_when_files_lists_directory() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join(".npmignore"), "dist/\n");
    write(fixture.path().join("dist/index.js"), "export {}\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "root .npmignore should not override package.json files entries"
    );
}

#[test]
fn npmignore_files_check_reports_nested_npmignore_excluding_included_file() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_expands_files_globs_before_nested_npmignore() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist/**/*.js"] }"#,
    );
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_matches_nested_globstar_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(
        fixture.path().join("dist/components/index.js"),
        "export {}\n",
    );
    write(fixture.path().join("dist/.npmignore"), "**/*.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/components/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_respects_anchored_nested_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(
        fixture.path().join("dist/components/index.js"),
        "export {}\n",
    );
    write(fixture.path().join("dist/.npmignore"), "/index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_eq!(
        outcome.findings.len(),
        1,
        "anchored nested .npmignore pattern should match only the nested ignore root"
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_matches_multi_star_segment_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/foo.test.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "*.test.*\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/foo.test.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_expands_files_globs_that_match_directories() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist/*"] }"#,
    );
    write(fixture.path().join("dist/foo/index.js"), "export {}\n");
    write(fixture.path().join("dist/foo/.npmignore"), "index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/foo/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/foo/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_expands_question_mark_files_globs() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist/index.?s"] }"#,
    );
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_matches_nested_basename_directory_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(
        fixture.path().join("dist/components/__tests__/fixture.js"),
        "export {}\n",
    );
    write(fixture.path().join("dist/.npmignore"), "__tests__\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/components/__tests__/fixture.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_does_not_match_directory_only_pattern_to_same_named_file() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/foo"), "published file\n");
    write(fixture.path().join("dist/.npmignore"), "foo/\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "directory-only nested .npmignore pattern should not exclude a same-named file"
    );
}

#[test]
fn npmignore_files_check_still_matches_directory_only_pattern_contents() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/foo/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "foo/\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/foo/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_matches_anchored_basename_pattern_contents() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/foo/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "/foo\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "npmignore-files:{}:dist/foo/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn npmignore_files_check_does_not_unescape_bang_as_negation_prefix() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/secret.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "\\!secret.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "escaped bang pattern should match literal !secret.js, not secret.js"
    );
}

#[test]
fn npmignore_files_check_respects_later_nested_negation() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "*.js\n!index.js\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_npmignore_files_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "later negated nested .npmignore pattern should keep explicit files entry quiet"
    );
}

#[test]
fn audit_project_runs_npmignore_files_check_for_gitignored_package_files_entries() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["dist"] }"#,
    );
    write(fixture.path().join(".gitignore"), "dist/\n");
    write(fixture.path().join("dist/index.js"), "export {}\n");
    write(fixture.path().join("dist/.npmignore"), "index.js\n");

    let mut config = MaximusConfig::default();
    config.gitignore_patterns = vec!["dist/".to_string()];
    config.checks.only = vec!["npmignore-files".to_string()];

    let audited =
        audit_project_with_config(fixture.path(), &config).expect("audit should run with config");

    assert_has_finding(
        &audited.result.findings,
        &format!(
            "npmignore-files:{}:dist/index.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Warn,
        "package.json files entry is excluded by nested .npmignore",
        Some(fixture.path().join("dist/.npmignore")),
    );
}

#[test]
fn audit_project_npmignore_files_check_respects_config_ignored_files_entries() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["ignored"] }"#,
    );
    write(fixture.path().join("ignored/index.js"), "export {}\n");
    write(fixture.path().join("ignored/.npmignore"), "index.js\n");

    let mut config = MaximusConfig::default();
    config.ignore_patterns = vec!["ignored".to_string()];
    config.checks.only = vec!["npmignore-files".to_string()];

    let audited =
        audit_project_with_config(fixture.path(), &config).expect("audit should run with config");

    assert!(
        audited.result.findings.is_empty(),
        "config ignored package files entries should not produce npmignore-files findings: {:?}",
        audited.result.findings
    );
}

#[test]
fn audit_project_runs_npmignore_files_check() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "files": ["bin"] }"#,
    );
    write(fixture.path().join("bin/cli.js"), "#!/usr/bin/env node\n");
    write(fixture.path().join("bin/.npmignore"), "cli.js\n");

    let audited = audit_project(fixture.path()).expect("audit should run");

    assert!(
        audited.result.findings.iter().any(|finding| finding.id
            == format!(
                "npmignore-files:{}:bin/cli.js",
                fixture.path().join("package.json").to_string_lossy()
            )),
        "registered audit path should include npmignore-files findings"
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
