use std::fs;
use std::path::Path;

use maximus_checks::{
    run_jsx_config_check, run_module_system_check, run_monorepo_tsconfig_check,
    run_registered_checks,
};
use maximus_core::{discover_project, Severity};
use tempfile::TempDir;

#[test]
fn module_system_check_reports_module_type_conflict_and_stays_quiet_when_aligned() {
    let conflict = TempDir::new().expect("temp dir should exist");

    write(
        conflict.path().join("package.json"),
        r#"{ "type": "module" }"#,
    );
    write(
        conflict.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "module": "commonjs" } }"#,
    );

    let project = discover_project(conflict.path()).expect("project should discover");
    let outcome = run_module_system_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("module-system:{}", conflict.path().join("tsconfig.json").to_string_lossy()),
        Severity::Error,
        "Package ESM type conflicts with tsconfig module output",
        "package.json type is \"module\", but compilerOptions.module is \"commonjs\".",
        "Use an ESM-aware module target or switch package.json type back to commonjs so the runtime and compiler agree.",
        Some(conflict.path().join("tsconfig.json")),
    );

    let aligned = TempDir::new().expect("temp dir should exist");

    write(
        aligned.path().join("package.json"),
        r#"{ "type": "module" }"#,
    );
    write(
        aligned.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "module": "nodenext" } }"#,
    );

    let project = discover_project(aligned.path()).expect("project should discover");
    let outcome = run_module_system_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());

    let commonjs_nodenext = TempDir::new().expect("temp dir should exist");

    write(
        commonjs_nodenext.path().join("package.json"),
        r#"{ "type": "commonjs" }"#,
    );
    write(
        commonjs_nodenext.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "module": "nodenext" } }"#,
    );

    let project = discover_project(commonjs_nodenext.path()).expect("project should discover");
    let outcome = run_module_system_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());
}

#[test]
fn module_system_check_reads_inherited_module_setting_from_extended_tsconfig() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "type": "module" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "module": "commonjs" } }"#,
    );
    write(
        fixture.path().join("tsconfig.json"),
        r#"{ "extends": "./tsconfig.base.json" }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_module_system_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("module-system:{}", fixture.path().join("tsconfig.json").to_string_lossy()),
        Severity::Error,
        "Package ESM type conflicts with tsconfig module output",
        "package.json type is \"module\", but compilerOptions.module is \"commonjs\".",
        "Use an ESM-aware module target or switch package.json type back to commonjs so the runtime and compiler agree.",
        Some(fixture.path().join("tsconfig.json")),
    );
}

#[test]
fn monorepo_tsconfig_check_reports_drift_and_accepts_shared_base_extends() {
    let conflict = TempDir::new().expect("temp dir should exist");

    write(
        conflict.path().join("package.json"),
        r#"{ "name": "repo", "private": true }"#,
    );
    write(
        conflict.path().join("packages/app/package.json"),
        r#"{ "name": "@repo/app" }"#,
    );
    write(
        conflict.path().join("packages/lib/package.json"),
        r#"{ "name": "@repo/lib" }"#,
    );
    write(
        conflict.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write(
        conflict.path().join("packages/app/tsconfig.json"),
        r#"{ "compilerOptions": { "target": "es2020" } }"#,
    );

    let project = discover_project(conflict.path()).expect("project should discover");
    let outcome = run_monorepo_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "monorepo-tsconfig-drift:{}",
            conflict.path().join("packages/app/tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Package tsconfig drifts from the shared base",
        "It does not extend a shared base config. package config should extend tsconfig.base.json so shared compiler settings stay aligned.",
        "Point package-level tsconfig files at the repo root tsconfig.base.json before adding local overrides.",
        Some(conflict.path().join("packages/app/tsconfig.json")),
    );

    let aligned = TempDir::new().expect("temp dir should exist");

    write(
        aligned.path().join("package.json"),
        r#"{ "name": "repo", "private": true }"#,
    );
    write(
        aligned.path().join("packages/app/package.json"),
        r#"{ "name": "@repo/app" }"#,
    );
    write(
        aligned.path().join("packages/lib/package.json"),
        r#"{ "name": "@repo/lib" }"#,
    );
    write(
        aligned.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write(
        aligned.path().join("packages/app/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.base.json", "compilerOptions": { "target": "es2020" } }"#,
    );

    let project = discover_project(aligned.path()).expect("project should discover");
    let outcome = run_monorepo_tsconfig_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());
}

#[test]
fn monorepo_tsconfig_check_ignores_auxiliary_package_configs_without_entry_config() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "name": "repo", "private": true }"#,
    );
    write(
        fixture.path().join("packages/app/package.json"),
        r#"{ "name": "@repo/app" }"#,
    );
    write(
        fixture.path().join("packages/lib/package.json"),
        r#"{ "name": "@repo/lib" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write(
        fixture.path().join("packages/app/tsconfig.base.json"),
        r#"{ "extends": "../../tsconfig.base.json" }"#,
    );
    write(
        fixture.path().join("packages/app/tsconfig.app.json"),
        r#"{ "extends": "./tsconfig.base.json" }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_monorepo_tsconfig_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn jsx_config_check_reports_framework_hint_and_accepts_explicit_import_source() {
    let conflict = TempDir::new().expect("temp dir should exist");

    write(
        conflict.path().join("package.json"),
        r#"{ "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        conflict.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "jsx": "react-jsx" } }"#,
    );

    let project = discover_project(conflict.path()).expect("project should discover");
    let outcome = run_jsx_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("jsx-config:{}", conflict.path().join("tsconfig.json").to_string_lossy()),
        Severity::Info,
        "preact JSX runtime should declare jsxImportSource",
        "package.json depends on preact, but compilerOptions.jsxImportSource is missing or different. No jsxImportSource is configured yet.",
        "Set compilerOptions.jsxImportSource to \"preact\" so the JSX transform matches the framework runtime.",
        Some(conflict.path().join("tsconfig.json")),
    );

    let aligned = TempDir::new().expect("temp dir should exist");

    write(
        aligned.path().join("package.json"),
        r#"{ "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        aligned.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "jsx": "react-jsx", "jsxImportSource": "preact" } }"#,
    );

    let project = discover_project(aligned.path()).expect("project should discover");
    let outcome = run_jsx_config_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());

    let classic_runtime = TempDir::new().expect("temp dir should exist");

    write(
        classic_runtime.path().join("package.json"),
        r#"{ "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        classic_runtime.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "jsx": "preserve" } }"#,
    );

    let project = discover_project(classic_runtime.path()).expect("project should discover");
    let outcome = run_jsx_config_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());
}

#[test]
fn jsx_config_check_reads_inherited_jsx_settings_from_extended_tsconfig() {
    let conflict = TempDir::new().expect("temp dir should exist");

    write(
        conflict.path().join("package.json"),
        r#"{ "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        conflict.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "jsx": "react-jsx" } }"#,
    );
    write(
        conflict.path().join("tsconfig.json"),
        r#"{ "extends": "./tsconfig.base.json" }"#,
    );

    let project = discover_project(conflict.path()).expect("project should discover");
    let outcome = run_jsx_config_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("jsx-config:{}", conflict.path().join("tsconfig.json").to_string_lossy()),
        Severity::Info,
        "preact JSX runtime should declare jsxImportSource",
        "package.json depends on preact, but compilerOptions.jsxImportSource is missing or different. No jsxImportSource is configured yet.",
        "Set compilerOptions.jsxImportSource to \"preact\" so the JSX transform matches the framework runtime.",
        Some(conflict.path().join("tsconfig.json")),
    );

    let aligned = TempDir::new().expect("temp dir should exist");

    write(
        aligned.path().join("package.json"),
        r#"{ "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        aligned.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "jsx": "react-jsx", "jsxImportSource": "preact" } }"#,
    );
    write(
        aligned.path().join("tsconfig.json"),
        r#"{ "extends": "./tsconfig.base.json" }"#,
    );

    let project = discover_project(aligned.path()).expect("project should discover");
    let outcome = run_jsx_config_check(&project).expect("check should run");
    assert!(outcome.findings.is_empty());
}

#[test]
fn module_and_jsx_checks_ignore_auxiliary_tsconfig_files() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "type": "commonjs", "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "module": "esnext", "jsx": "react-jsx" } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");

    let module_outcome = run_module_system_check(&project).expect("check should run");
    assert!(module_outcome.findings.is_empty());

    let jsx_outcome = run_jsx_config_check(&project).expect("check should run");
    assert!(jsx_outcome.findings.is_empty());
}

#[test]
fn registered_checks_include_new_coherence_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "name": "repo", "private": true, "type": "module", "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write(
        fixture.path().join("packages/app/package.json"),
        r#"{ "name": "@repo/app" }"#,
    );
    write(
        fixture.path().join("packages/lib/package.json"),
        r#"{ "name": "@repo/lib" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write(
        fixture.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "module": "commonjs", "jsx": "react-jsx" } }"#,
    );
    write(
        fixture.path().join("packages/app/tsconfig.json"),
        r#"{ "compilerOptions": { "target": "es2020" } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_registered_checks(&project).expect("registry should run");
    let finding_ids = outcome
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<Vec<_>>();

    assert!(finding_ids
        .iter()
        .any(|id| id.starts_with("module-system:")));
    assert!(finding_ids
        .iter()
        .any(|id| id.starts_with("monorepo-tsconfig-drift:")));
    assert!(finding_ids.iter().any(|id| id.starts_with("jsx-config:")));
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
