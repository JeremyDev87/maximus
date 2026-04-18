use std::fs;
use std::path::Path;

use maximus_core::{discover_project, Finding, Severity};
use tempfile::TempDir;

#[path = "../src/structure.rs"]
mod structure;

use structure::build_structure_report;

#[test]
fn structure_report_recommends_shared_tsconfig_for_monorepos_without_base_config() {
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
        fixture.path().join("packages/app/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.json" }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let report = build_structure_report(&project, &[]);

    assert!(report.is_monorepo);
    assert_eq!(report.package_count, 3);
    assert_eq!(report.env_directories, 0);
    assert_eq!(report.config_files, 4);
    assert_eq!(
        report.recommendations,
        vec![
            "Introduce a shared tsconfig.base.json so packages inherit one source of truth."
                .to_string(),
            "Current config surface looks healthy. Keep shared rules centralized as the repo grows."
                .to_string(),
        ]
    );
}

#[test]
fn structure_report_skips_shared_tsconfig_recommendation_when_base_file_exists() {
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
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let report = build_structure_report(&project, &[sample_finding()]);

    assert!(report.is_monorepo);
    assert_eq!(report.package_count, 2);
    assert!(report.recommendations.is_empty());
}

#[test]
fn structure_report_flags_multiple_eslint_entry_points_with_js_wording() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "name": "repo", "private": true }"#,
    );
    write(fixture.path().join(".eslintrc.json"), "{}");
    write(
        fixture.path().join("eslint.config.js"),
        "export default [];\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let report = build_structure_report(&project, &[sample_finding()]);

    assert_eq!(
        report.recommendations,
        vec![
            "Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets."
                .to_string(),
        ]
    );
}

#[test]
fn structure_report_flags_env_directories_without_root_example_contract() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "name": "repo", "private": true }"#,
    );
    write(
        fixture.path().join("apps/web/.env"),
        "API_URL=https://example.com\n",
    );
    write(
        fixture.path().join("packages/api/.env.production"),
        "TOKEN=secret\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let report = build_structure_report(&project, &[sample_finding()]);

    assert_eq!(report.env_directories, 2);
    assert_eq!(
        report.recommendations,
        vec![
            "Use .env.example files consistently so onboarding does not depend on tribal knowledge."
                .to_string(),
        ]
    );
}

#[test]
fn structure_report_preserves_clean_project_recommendation_when_no_findings_exist() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "name": "clean-project", "private": true }"#,
    );
    write(
        fixture.path().join(".env.example"),
        "API_URL=https://example.com\n",
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let report = build_structure_report(&project, &[]);

    assert!(!report.is_monorepo);
    assert_eq!(report.package_count, 1);
    assert_eq!(report.env_directories, 1);
    assert_eq!(report.config_files, 3);
    assert_eq!(
        report.recommendations,
        vec![
            "Current config surface looks healthy. Keep shared rules centralized as the repo grows."
                .to_string(),
        ]
    );
}

fn sample_finding() -> Finding {
    Finding {
        id: "sample".to_string(),
        severity: Severity::Warn,
        category: "test".to_string(),
        title: "sample".to_string(),
        detail: "sample".to_string(),
        file: None,
        hint: "sample".to_string(),
        fixable: false,
        fix_ids: Vec::new(),
    }
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}
