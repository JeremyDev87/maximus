use std::fs;
use std::path::Path;

use maximus_checks::{
    run_config_duplicate_check, run_eslint_prettier_check, run_registered_checks,
};
use maximus_core::{discover_project, Severity};
use tempfile::TempDir;

#[test]
fn config_duplicate_check_matches_js_duplicate_and_mixed_mode_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"
        {
          "eslintConfig": { "root": true },
          "prettier": { "semi": true },
          "jest": { "testEnvironment": "node" }
        }
        "#,
    );
    write(fixture.path().join(".eslintrc.json"), "{}");
    write(fixture.path().join("eslint.config.js"), "export default [];\n");
    write(fixture.path().join(".prettierrc.json"), "{}");
    write(fixture.path().join("jest.config.js"), "export default {};\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_config_duplicate_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("duplicate-config:ESLint:{}", fixture.path().to_string_lossy()),
        Severity::Error,
        "ESLint config is declared in multiple places",
        "Found 3 ESLint config sources in ..",
        "Keep a single ESLint entry point per directory to avoid drift.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!("duplicate-config:Prettier:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Prettier config is declared in multiple places",
        "Found 2 Prettier config sources in ..",
        "Keep a single Prettier entry point per directory to avoid drift.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!("duplicate-config:Jest:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Jest config is declared in multiple places",
        "Found 2 Jest config sources in ..",
        "Keep a single Jest entry point per directory to avoid drift.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!("eslint-mixed-modes:{}", fixture.path().to_string_lossy()),
        Severity::Error,
        "Legacy and flat ESLint configs coexist",
        "ESLint may resolve different config systems depending on invocation and toolchain.",
        "Pick either flat config (eslint.config.*) or legacy .eslintrc.* in the same directory.",
        Some(fixture.path().join(".eslintrc.json")),
    );
}

#[test]
fn eslint_prettier_check_reports_conflict_when_formatting_rules_lack_bridge() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("eslint.config.js"),
        r#"export default [{ rules: { "quotes": ["error", "single"], "semi": "error" } }];"#,
    );
    write(fixture.path().join(".prettierrc.json"), "{ \"semi\": true }\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_eslint_prettier_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("eslint-prettier-conflict:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "ESLint formatting rules may conflict with Prettier",
        "Formatting-oriented ESLint rules were found, but no explicit Prettier bridge was detected.",
        "Consider eslint-config-prettier or plugin:prettier/recommended to reduce formatter churn.",
        Some(fixture.path().join("eslint.config.js")),
    );
}

#[test]
fn eslint_prettier_check_reports_separate_info_when_no_bridge_is_declared() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(fixture.path().join("eslint.config.js"), "export default [];\n");
    write(fixture.path().join(".prettierrc.json"), "{ \"semi\": true }\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_eslint_prettier_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("eslint-prettier-separate:{}", fixture.path().to_string_lossy()),
        Severity::Info,
        "ESLint and Prettier are configured separately",
        "That can be fine, but teams often prefer an explicit integration strategy.",
        "Document which tool owns formatting and which tool owns code-quality rules.",
        Some(fixture.path().join("eslint.config.js")),
    );
}

#[test]
fn eslint_prettier_check_ignores_falsy_package_json_entries_like_js() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "eslintConfig": false, "prettier": { "semi": true } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_eslint_prettier_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn duplicate_config_detail_uses_js_style_relative_path_rendering() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("packages/app/package.json"),
        r#"{ "prettier": { "semi": true } }"#,
    );
    write(fixture.path().join("packages/app/.prettierrc.json"), "{}");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_config_duplicate_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "duplicate-config:Prettier:{}",
            fixture.path().join("packages/app").to_string_lossy()
        ),
        Severity::Warn,
        "Prettier config is declared in multiple places",
        &format!(
            "Found 2 Prettier config sources in {}.",
            expected_js_relative_dir("packages/app")
        ),
        "Keep a single Prettier entry point per directory to avoid drift.",
        Some(fixture.path().join("packages/app/package.json")),
    );
}

#[test]
fn registered_checks_aggregate_duplicate_and_conflict_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "eslintConfig": { "root": true }, "prettier": { "semi": true } }"#,
    );
    write(fixture.path().join(".eslintrc.json"), "{}");
    write(
        fixture.path().join("eslint.config.js"),
        r#"export default [{ rules: { "quotes": ["error", "single"] } }];"#,
    );
    write(fixture.path().join(".prettierrc.json"), "{}");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_registered_checks(&project).expect("registry should run");

    let finding_ids = outcome
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        finding_ids,
        vec![
            format!("eslint-mixed-modes:{}", fixture.path().to_string_lossy()),
            format!("duplicate-config:ESLint:{}", fixture.path().to_string_lossy()),
            format!("eslint-prettier-conflict:{}", fixture.path().to_string_lossy()),
            format!("duplicate-config:Prettier:{}", fixture.path().to_string_lossy()),
        ]
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn expected_js_relative_dir(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
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
