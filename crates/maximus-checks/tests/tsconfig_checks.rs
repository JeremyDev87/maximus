use std::fs;
use std::path::{Path, PathBuf};

use maximus_core::{discover_project, Severity};
use tempfile::TempDir;

#[path = "../src/tsconfig.rs"]
mod tsconfig;

use tsconfig::run_tsconfig_check;

#[test]
fn tsconfig_check_reports_parse_and_deprecated_option_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "charset": "utf8",
            "importsNotUsedAsValues": "remove",
            "out": "./dist/index.js"
          }
        }
        "#,
    );
    write(
        fixture.path().join("tsconfig.bad.json"),
        r#"{ "compilerOptions": { "paths": { "@bad/*": ["src/*",] } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-deprecated:{}:charset",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Deprecated compiler option \"charset\"",
        "Remove it. TypeScript ignores this option in modern versions.",
        "Remove legacy flags before they become upgrade blockers.",
        Some(fixture.path().join("tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-deprecated:{}:importsNotUsedAsValues",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Deprecated compiler option \"importsNotUsedAsValues\"",
        "Prefer verbatimModuleSyntax in modern TypeScript.",
        "Remove legacy flags before they become upgrade blockers.",
        Some(fixture.path().join("tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-deprecated:{}:out",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Deprecated compiler option \"out\"",
        "Use outFile if you truly need single-file emit.",
        "Remove legacy flags before they become upgrade blockers.",
        Some(fixture.path().join("tsconfig.json")),
    );

    let parse_finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-parse:{}",
                    fixture.path().join("tsconfig.bad.json").to_string_lossy()
                )
        })
        .expect("parse finding should exist");
    assert_eq!(parse_finding.severity, Severity::Error);
    assert_eq!(parse_finding.title, "Config file could not be parsed");
    assert_eq!(
        parse_finding.hint,
        "Fix invalid JSONC syntax before relying on this config."
    );
    assert_eq!(
        parse_finding.file,
        Some(fixture.path().join("tsconfig.bad.json"))
    );
    assert!(
        !parse_finding.detail.is_empty(),
        "parse error detail should preserve parser output"
    );
}

#[test]
fn tsconfig_check_reports_paths_shape_type_empty_wildcard_and_missing_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("shape/tsconfig.json"),
        r#"{ "compilerOptions": { "paths": [] } }"#,
    );
    write(
        fixture.path().join("paths/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "@empty/*": [],
              "@typed/*": [42],
              "@mismatch/*": ["src/mismatch"],
              "@missing/*": ["src/missing/*"],
              "@ok": ["src/existing/index"],
              "@ok-wild/*": ["src/generated/*"],
              "@external": ["@scope/pkg"]
            }
          }
        }
        "#,
    );
    write(
        fixture.path().join("paths/src/existing/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("paths/src/generated/placeholder.txt"),
        "seed\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-shape:{}",
            fixture.path().join("shape/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "compilerOptions.paths must be an object",
        "TypeScript expects alias keys mapped to arrays of target strings.",
        "Rewrite paths to the standard { alias: [targets] } shape.",
        Some(fixture.path().join("shape/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-empty:{}:@empty/*",
            fixture.path().join("paths/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Alias \"@empty/*\" does not declare any targets",
        "Each path alias should map to at least one target string.",
        "Add a valid target or remove the alias entry.",
        Some(fixture.path().join("paths/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-type:{}:@typed/*",
            fixture.path().join("paths/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Alias \"@typed/*\" contains a non-string target",
        "TypeScript path targets must be strings.",
        "Replace non-string entries with valid path strings.",
        Some(fixture.path().join("paths/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-wildcard:{}:@mismatch/*:src/mismatch",
            fixture.path().join("paths/tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Wildcard shape does not match for alias \"@mismatch/*\"",
        "@mismatch/* maps to src/mismatch, but only one side uses \"*\".",
        "Keep wildcard placement aligned so imports resolve predictably.",
        Some(fixture.path().join("paths/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-missing:{}:@missing/*:src/missing/*",
            fixture.path().join("paths/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Path alias target does not exist",
        "@missing/* points to src/missing/*, but the resolved path was not found.",
        "Update or remove stale aliases before they break editor and build resolution.",
        Some(fixture.path().join("paths/tsconfig.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-missing:{}:@ok:src/existing/index",
                    fixture.path().join("paths/tsconfig.json").to_string_lossy()
                )
        }),
        "existing extension-less target should not be reported missing"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-missing:{}:@ok-wild/*:src/generated/*",
                    fixture.path().join("paths/tsconfig.json").to_string_lossy()
                )
        }),
        "existing wildcard directory should not be reported missing"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-missing:{}:@external:@scope/pkg",
                    fixture.path().join("paths/tsconfig.json").to_string_lossy()
                )
        }),
        "scoped package targets should bypass file existence checks"
    );
}

#[test]
fn tsconfig_check_compares_package_imports_like_js_contract() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#ok": "./src/index.ts",
            "#cond": {
              "types": "./src/feature.ts",
              "default": "./src/feature.ts"
            },
            "#mismatch": {
              "default": "./src/runtime.ts"
            },
            "#ignored": false
          }
        }
        "##,
    );
    write(
        fixture.path().join("src/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("src/feature.ts"),
        "export const feature = true;\n",
    );
    write(
        fixture.path().join("src/runtime.ts"),
        "export const runtime = true;\n",
    );
    write(
        fixture.path().join("src/editor.ts"),
        "export const editor = true;\n",
    );
    write(
        fixture.path().join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "#ok": ["./src/index.ts"],
              "#cond": ["./src/feature.ts"],
              "#mismatch": ["./src/editor.ts"],
              "#ignored": ["./src/index.ts"]
            }
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-import-conflict:{}:#mismatch",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Alias \"#mismatch\" differs between tsconfig and package imports",
        "tsconfig resolves to ./src/editor.ts, while package.json imports resolves to {\"default\":\"./src/runtime.ts\"}.",
        "Align both alias surfaces so runtime and editor resolution stay consistent.",
        Some(fixture.path().join("tsconfig.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-import-conflict:{}:#ok",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "matching string imports should not report a conflict"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-import-conflict:{}:#cond",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "matching conditional imports should not report a conflict"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-import-conflict:{}:#ignored",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "non-object and non-string import targets should be ignored"
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
    detail: &str,
    hint: &str,
    file: Option<PathBuf>,
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
