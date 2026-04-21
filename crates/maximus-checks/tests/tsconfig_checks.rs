use std::fs;
use std::path::{Path, PathBuf};

use maximus_core::{discover_project, Severity};
use tempfile::TempDir;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[path = "../src/tsconfig.rs"]
mod tsconfig;
#[path = "../src/check_outcome.rs"]
mod check_outcome;

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
fn tsconfig_check_treats_null_paths_like_missing_paths() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "paths": null } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "null paths should be ignored like the JS reference implementation"
    );
    assert!(outcome.fixes.is_empty());
    assert!(outcome.planned_fixes.is_empty());
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

#[test]
fn tsconfig_check_uses_first_existing_paths_target_for_import_comparison() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#alias": "./src/index.ts"
          }
        }
        "##,
    );
    write(
        fixture.path().join("src/wrong.ts"),
        "export const wrong = true;\n",
    );
    write(
        fixture.path().join("src/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "#alias": ["./src/wrong.ts", "./src/index.ts"]
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
            "tsconfig-import-conflict:{}:#alias",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Warn,
        "Alias \"#alias\" differs between tsconfig and package imports",
        "tsconfig resolves to ./src/wrong.ts, while package.json imports resolves to ./src/index.ts.",
        "Align both alias surfaces so runtime and editor resolution stay consistent.",
        Some(fixture.path().join("tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_treats_paths_targets_as_fallback_candidates() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#alias": "./src/index.ts"
          }
        }
        "##,
    );
    write(
        fixture.path().join("src/index.ts"),
        "export const fallback = true;\n",
    );
    write(
        fixture.path().join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "#alias": ["./src/generated.ts", "./src/index.ts"]
            }
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");
    let tsconfig_path = fixture.path().join("tsconfig.json");

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-missing:{}:#alias:./src/generated.ts",
                    tsconfig_path.to_string_lossy()
                )
        }),
        "missing earlier fallback candidates should not report a missing-target error"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-import-conflict:{}:#alias",
                    tsconfig_path.to_string_lossy()
                )
        }),
        "imports should match any viable tsconfig fallback target"
    );
}

#[test]
fn tsconfig_check_reports_project_reference_missing_and_composite_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "./packages/pkg-a" },
            { "path": "./packages/pkg-b" },
            { "path": "./packages/missing" },
            { "path": "./packages/pkg-c/tsconfig.build.json" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );
    write(
        fixture.path().join("packages/pkg-b/tsconfig.json"),
        r#"{ "compilerOptions": { "declaration": true } }"#,
    );
    write(
        fixture.path().join("packages/pkg-c/tsconfig.build.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:./packages/missing:missing",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target does not exist",
        "./packages/missing does not resolve to an existing tsconfig file.",
        "Update stale project references before they break TypeScript build mode.",
        Some(fixture.path().join("tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:./packages/pkg-b:composite",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Referenced project must enable composite",
        &format!(
            "./packages/pkg-b resolves to {}, but compilerOptions.composite is not true.",
            fixture
                .path()
                .join("packages/pkg-b/tsconfig.json")
                .to_string_lossy()
        ),
        "Enable composite on referenced projects so TypeScript build mode can consume them reliably.",
        Some(fixture.path().join("tsconfig.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:./packages/pkg-a:composite",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "composite-enabled directory references should not report a warning"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:./packages/pkg-c/tsconfig.build.json:composite",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "composite-enabled file references should not report a warning"
    );
}

#[test]
fn tsconfig_check_reports_invalid_composite_value_types() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "compilerOptions": { "composite": "true" }, "extends": "../../tsconfig.base.json" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../packages/pkg-a:extends-composite-type",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("invalid composite type finding should exist");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(
        finding.title,
        "Referenced project must set compilerOptions.composite to a boolean"
    );
    assert!(
        finding.detail.contains("packages/pkg-a/tsconfig.json"),
        "detail should preserve the referenced config path"
    );
    assert_eq!(
        finding.hint,
        "Set compilerOptions.composite to true or false before relying on project references."
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "invalid composite value types should not degrade into generic composite findings"
    );
}

#[test]
fn tsconfig_check_does_not_resolve_extensionless_reference_paths_to_sibling_json_files() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:../packages/pkg-a:missing",
            fixture.path().join("root/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target does not exist",
        "../packages/pkg-a does not resolve to an existing tsconfig file.",
        "Update stale project references before they break TypeScript build mode.",
        Some(fixture.path().join("root/tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_respects_inherited_composite_for_project_references() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.base.json" }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "project references that inherit composite through extends should not report a composite error"
    );
}

#[test]
fn tsconfig_check_respects_package_based_extends_for_project_references() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("node_modules/@tsconfig/shared/tsconfig.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "extends": "@tsconfig/shared/tsconfig.json" }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "project references that inherit composite through package-based extends should not report a composite error"
    );
}

#[test]
fn tsconfig_check_reports_inherited_parse_errors_before_composite_missing() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.base.json" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "compilerOptions": { "composite": true, }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../packages/pkg-a:extends-parse",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("extends parse finding should exist");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.title, "Inherited tsconfig could not be parsed");
    assert!(
        finding.detail.contains("tsconfig.base.json"),
        "detail should preserve the inherited config path"
    );
    assert_eq!(
        finding.hint,
        "Fix invalid JSONC syntax in extended tsconfig files before relying on inherited composite settings."
    );
    assert_eq!(finding.file, Some(fixture.path().join("root/tsconfig.json")));
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "inherited parse failures should not degrade into composite findings"
    );
}

#[test]
fn tsconfig_check_reports_missing_inherited_configs_before_composite_missing() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.base.json" }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../packages/pkg-a:extends-missing",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("extends missing finding should exist");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.title, "Inherited tsconfig could not be found");
    assert!(
        finding.detail.contains("../../tsconfig.base.json"),
        "detail should preserve the unresolved extends path"
    );
    assert_eq!(
        finding.hint,
        "Make sure extends points at an existing tsconfig-style file before relying on inherited composite settings."
    );
    assert_eq!(finding.file, Some(fixture.path().join("root/tsconfig.json")));
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "missing inherited configs should not degrade into composite findings"
    );
}

#[test]
fn tsconfig_check_reports_extends_cycles_before_composite_missing() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "extends": "../../tsconfig.base.json" }"#,
    );
    write(
        fixture.path().join("tsconfig.base.json"),
        r#"{ "extends": "./packages/pkg-a/tsconfig.json" }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../packages/pkg-a:extends-cycle",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("extends cycle finding should exist");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.title, "Inherited tsconfig extends cycle detected");
    assert!(
        finding.detail.contains("packages/pkg-a/tsconfig.json")
            || finding.detail.contains("tsconfig.base.json"),
        "detail should preserve one of the cyclic config paths"
    );
    assert_eq!(
        finding.hint,
        "Break extends cycles before relying on inherited composite settings."
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a:composite",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "extends cycles should not degrade into generic composite findings"
    );
}

#[test]
fn tsconfig_check_reports_unparseable_project_reference_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("pkg-a/tsconfig.json"),
        r#"{ "compilerOptions": { "paths": { "@bad/*": ["src/*",] } }"#,
    );

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let parse_finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../pkg-a:parse",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("reference parse finding should exist");
    assert_eq!(parse_finding.severity, Severity::Error);
    assert_eq!(
        parse_finding.title,
        "Project reference target could not be parsed"
    );
    assert_eq!(
        parse_finding.hint,
        "Fix invalid JSONC syntax in referenced tsconfig files before relying on project references."
    );
    assert_eq!(
        parse_finding.file,
        Some(fixture.path().join("root/tsconfig.json"))
    );
    assert!(
        parse_finding
            .detail
            .contains("pkg-a/tsconfig.json"),
        "parse detail should preserve the referenced target path"
    );
}

#[test]
fn tsconfig_check_reports_non_tsconfig_reference_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "./package.json" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("package.json"),
        r#"{ "compilerOptions": { "composite": true } }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:./package.json:invalid-target",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target must point to a tsconfig file",
        &format!(
            "./package.json resolves to {}, but that file does not look like a tsconfig document.",
            fixture.path().join("package.json").to_string_lossy()
        ),
        "Point project references at a directory with tsconfig.json or an explicit tsconfig-style JSON file.",
        Some(fixture.path().join("tsconfig.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:./package.json:composite",
                    fixture.path().join("tsconfig.json").to_string_lossy()
                )
        }),
        "non-tsconfig files should not degrade into composite warnings"
    );
}

#[test]
fn tsconfig_check_reports_project_reference_shape_errors() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("shape/tsconfig.json"),
        r#"{ "references": { "path": "../pkg-a" } }"#,
    );
    write(
        fixture.path().join("entries/tsconfig.json"),
        r#"
        {
          "references": [
            false,
            {},
            { "path": "" }
          ]
        }
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references-shape:{}",
            fixture.path().join("shape/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "references must be an array",
        "TypeScript project references must use an array of { path } entries.",
        "Rewrite references to the standard [{ \"path\": \"../pkg\" }] shape.",
        Some(fixture.path().join("shape/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references-entry:{}:0",
            fixture.path().join("entries/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Each project reference entry must be an object with a path",
        "references[0] must be an object like { \"path\": \"../pkg\" }.",
        "Replace malformed reference entries with explicit { path } objects.",
        Some(fixture.path().join("entries/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references-entry:{}:1",
            fixture.path().join("entries/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Each project reference entry must declare a string path",
        "references[1] must declare a non-empty string path.",
        "Use { \"path\": \"../pkg\" } entries so TypeScript can resolve referenced projects.",
        Some(fixture.path().join("entries/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references-entry:{}:2",
            fixture.path().join("entries/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Each project reference entry must declare a string path",
        "references[2] must declare a non-empty string path.",
        "Use { \"path\": \"../pkg\" } entries so TypeScript can resolve referenced projects.",
        Some(fixture.path().join("entries/tsconfig.json")),
    );
}

#[cfg(unix)]
#[test]
fn tsconfig_check_reports_unreadable_project_reference_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target_path = fixture.path().join("packages/pkg-a/tsconfig.json");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a" }
          ]
        }
        "#,
    );
    write(&target_path, r#"{ "compilerOptions": { "composite": true } }"#);

    let original_permissions = fs::metadata(&target_path)
        .expect("target metadata should exist")
        .permissions();
    let mut unreadable_permissions = original_permissions.clone();
    unreadable_permissions.set_mode(0o000);
    fs::set_permissions(&target_path, unreadable_permissions)
        .expect("target permissions should update");

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let mut restore_permissions = original_permissions;
    restore_permissions.set_mode(0o644);
    fs::set_permissions(&target_path, restore_permissions)
        .expect("target permissions should restore");

    let finding = outcome
        .findings
        .iter()
        .find(|finding| {
            finding.id
                == format!(
                    "tsconfig-references:{}:../packages/pkg-a:unreadable",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        })
        .expect("unreadable finding should exist");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.title, "Project reference target could not be read");
    assert!(
        finding.detail.contains("Permission denied"),
        "detail should preserve the underlying permission error"
    );
}

#[test]
fn tsconfig_check_reports_semantically_invalid_tsconfig_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "./packages/pkg-a" },
            { "path": "./packages/pkg-b" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "name": "not-a-tsconfig" }"#,
    );
    write(
        fixture.path().join("packages/pkg-b/tsconfig.json"),
        r#"[1, 2, 3]"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:./packages/pkg-a:invalid-target",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target must point to a tsconfig file",
        &format!(
            "./packages/pkg-a resolves to {}, but that file does not look like a tsconfig document.",
            fixture
                .path()
                .join("packages/pkg-a/tsconfig.json")
                .to_string_lossy()
        ),
        "Point project references at a directory with tsconfig.json or an explicit tsconfig-style JSON file.",
        Some(fixture.path().join("tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:./packages/pkg-b:invalid-target",
            fixture.path().join("tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target must point to a tsconfig file",
        &format!(
            "./packages/pkg-b resolves to {}, but that file does not look like a tsconfig document.",
            fixture
                .path()
                .join("packages/pkg-b/tsconfig.json")
                .to_string_lossy()
        ),
        "Point project references at a directory with tsconfig.json or an explicit tsconfig-style JSON file.",
        Some(fixture.path().join("tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_accepts_empty_json_reference_targets_with_explicit_file_names() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a/build.json" }
          ]
        }
        "#,
    );
    write(fixture.path().join("packages/pkg-a/build.json"), "{}");

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a/build.json:invalid-target",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "explicit JSON config file names should be accepted even when the document is empty"
    );
}

#[test]
fn tsconfig_check_rejects_empty_generic_json_reference_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a/schema.json" }
          ]
        }
        "#,
    );
    write(fixture.path().join("packages/pkg-a/schema.json"), "{}");

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-references:{}:../packages/pkg-a/schema.json:invalid-target",
            fixture.path().join("root/tsconfig.json").to_string_lossy()
        ),
        Severity::Error,
        "Project reference target must point to a tsconfig file",
        &format!(
            "../packages/pkg-a/schema.json resolves to {}, but that file does not look like a tsconfig document.",
            fixture.path().join("packages/pkg-a/schema.json").to_string_lossy()
        ),
        "Point project references at a directory with tsconfig.json or an explicit tsconfig-style JSON file.",
        Some(fixture.path().join("root/tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_accepts_empty_reference_targets_with_non_json_file_names() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("root/tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "../packages/pkg-a/build" }
          ]
        }
        "#,
    );
    write(fixture.path().join("packages/pkg-a/build"), "{}");

    let project = discover_project(fixture.path().join("root").as_path())
        .expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-references:{}:../packages/pkg-a/build:invalid-target",
                    fixture.path().join("root/tsconfig.json").to_string_lossy()
                )
        }),
        "explicit non-json config file names should be accepted when referenced directly"
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
