use std::fs;
use std::path::{Path, PathBuf};

use maximus_core::{discover_project, summarize_findings, Severity, StructureReport};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[path = "../src/check_outcome.rs"]
mod check_outcome;
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
fn tsconfig_check_reports_path_alias_shadowing_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("src/shims/react.ts"),
        "export const reactShim = true;\n",
    );
    write(
        fixture.path().join("src/vendor/pkg/index.ts"),
        "export const pkg = true;\n",
    );
    write(
        fixture.path().join("src/app/index.ts"),
        "export const app = true;\n",
    );
    write(
        fixture.path().join("src/testing/index.ts"),
        "export const testing = true;\n",
    );
    write(
        fixture.path().join("src/utils/client.ts"),
        "export const client = true;\n",
    );
    write(
        fixture.path().join("src/hash/internal.ts"),
        "export const internal = true;\n",
    );
    write(
        fixture.path().join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "react": ["./src/shims/react.ts"],
              "@scope/pkg/*": ["./src/vendor/pkg/*"],
              "@app/*": ["./src/app/*"],
              "@app/testing": ["./src/testing/index.ts"],
              "@app/utils": ["./src/utils/client.ts"],
              "#internal/*": ["./src/hash/*"]
            }
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");
    let tsconfig_path = fixture.path().join("tsconfig.json");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-shadow-package:{}:react",
            tsconfig_path.to_string_lossy()
        ),
        Severity::Warn,
        "Path alias \"react\" shadows a package import",
        "\"react\" is a bare package-style specifier, so this alias can override an installed package or workspace package with the same import path.",
        "Prefer #internal/* or another dedicated namespace for app-local aliases so package imports stay unambiguous.",
        Some(tsconfig_path.clone()),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-paths-shadow-package:{}:@scope/pkg/*",
            tsconfig_path.to_string_lossy()
        ),
        Severity::Warn,
        "Path alias \"@scope/pkg/*\" shadows a package import",
        "\"@scope/pkg/*\" is a bare package-style specifier, so this alias can override an installed package or workspace package with the same import path.",
        "Prefer #internal/* or another dedicated namespace for app-local aliases so package imports stay unambiguous.",
        Some(tsconfig_path.clone()),
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-shadow-alias:{}:@app/testing:@app/*",
                    tsconfig_path.to_string_lossy()
                )
        }),
        "exact aliases should be allowed to specialize a broader wildcard alias"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-shadow-alias:{}:@app/utils:@app/*",
                    tsconfig_path.to_string_lossy()
                )
        }),
        "exact aliases should be allowed to specialize a broader wildcard alias"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id
                != format!(
                    "tsconfig-paths-shadow-package:{}:#internal/*",
                    tsconfig_path.to_string_lossy()
                )
        }),
        "#-prefixed aliases should not be treated as package imports"
    );
}

#[test]
fn tsconfig_check_reports_types_and_typeroots_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("types-only/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "types": ["node", "jest"]
          }
        }
        "#,
    );
    write(
        fixture.path().join("empty-types/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "types": []
          }
        }
        "#,
    );
    write(
        fixture.path().join("type-roots/types/custom/index.d.ts"),
        "export {};\n",
    );
    write(
        fixture.path().join("type-roots/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "typeRoots": ["./types", "./missing-types"]
          }
        }
        "#,
    );
    write(
        fixture.path().join("types-and-roots/types/node/index.d.ts"),
        "export {};\n",
    );
    write(
        fixture.path().join("types-and-roots/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "types": ["node"],
            "typeRoots": ["./types"]
          }
        }
        "#,
    );
    write(
        fixture.path().join("invalid/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "types": "node",
            "typeRoots": [42, ""]
          }
        }
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    let types_only_path = fixture.path().join("types-only/tsconfig.json");
    assert_has_finding(
        &outcome.findings,
        &format!("tsconfig-types-guidance:{}", types_only_path.to_string_lossy()),
        Severity::Info,
        "compilerOptions.types limits ambient type packages",
        "compilerOptions.types only includes [\"node\",\"jest\"], so unlisted ambient @types packages will not be injected automatically.",
        "Keep this list in sync with every test and runtime package that should contribute global types.",
        Some(types_only_path),
    );

    let empty_types_path = fixture.path().join("empty-types/tsconfig.json");
    assert_has_finding(
        &outcome.findings,
        &format!("tsconfig-types-guidance:{}", empty_types_path.to_string_lossy()),
        Severity::Warn,
        "compilerOptions.types disables automatic @types inclusion",
        "compilerOptions.types is set to [], so TypeScript will not auto-include any ambient @types packages.",
        "Keep this list in sync with every test and runtime package that should contribute global types.",
        Some(empty_types_path),
    );

    let type_roots_path = fixture.path().join("type-roots/tsconfig.json");
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-typeroots-missing:{}:./missing-types",
            type_roots_path.to_string_lossy()
        ),
        Severity::Warn,
        "Configured typeRoots entry does not exist",
        "compilerOptions.typeRoots includes \"./missing-types\", but the resolved path was not found.",
        "Create the missing types directory or remove stale typeRoots entries before TypeScript silently skips expected declarations.",
        Some(type_roots_path.clone()),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-typeroots-guidance:{}",
            type_roots_path.to_string_lossy()
        ),
        Severity::Warn,
        "compilerOptions.typeRoots disables default @types discovery",
        "compilerOptions.typeRoots only searches [\"./types\",\"./missing-types\"], so TypeScript will stop using the default node_modules/@types lookup for this config.",
        "Include every required ambient types directory or remove typeRoots to restore default discovery.",
        Some(type_roots_path),
    );

    let combined_path = fixture.path().join("types-and-roots/tsconfig.json");
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-types-typeroots-guidance:{}",
            combined_path.to_string_lossy()
        ),
        Severity::Warn,
        "compilerOptions.types and typeRoots both narrow ambient type resolution",
        "compilerOptions.types only includes [\"node\"], and compilerOptions.typeRoots only searches [\"./types\"], so unlisted ambient packages outside those roots will be hidden from TypeScript.",
        "Keep both lists aligned with every global types package your runtime and tests rely on.",
        Some(combined_path),
    );

    let invalid_path = fixture.path().join("invalid/tsconfig.json");
    assert_has_finding(
        &outcome.findings,
        &format!("tsconfig-types-shape:{}", invalid_path.to_string_lossy()),
        Severity::Error,
        "\"compilerOptions.types\" must be an array of package names",
        "TypeScript expects compilerOptions.types to be an array of string package names.",
        "Rewrite compilerOptions.types as [\"node\", \"jest\"]-style package names or remove it.",
        Some(invalid_path.clone()),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-typeroots-entry:{}:0",
            invalid_path.to_string_lossy()
        ),
        Severity::Error,
        "\"compilerOptions.typeRoots\" contains a non-string path",
        &format!(
            "{} declares compilerOptions.typeRoots[0], but TypeScript expects a non-empty string directory path.",
            invalid_path.to_string_lossy()
        ),
        "Rewrite compilerOptions.typeRoots as [\"./types\", \"./node_modules/@types\"]-style paths or remove it.",
        Some(invalid_path.clone()),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-typeroots-entry:{}:1",
            invalid_path.to_string_lossy()
        ),
        Severity::Error,
        "\"compilerOptions.typeRoots\" contains a non-string path",
        &format!(
            "{} declares compilerOptions.typeRoots[1], but TypeScript expects a non-empty string directory path.",
            invalid_path.to_string_lossy()
        ),
        "Rewrite compilerOptions.typeRoots as [\"./types\", \"./node_modules/@types\"]-style paths or remove it.",
        Some(invalid_path),
    );
}

#[test]
fn tsconfig_check_reports_empty_include_and_useless_exclude_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("empty-include/tsconfig.json"),
        r#"
        {
          "include": ["src/missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("empty-include/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("empty-exclude/tsconfig.json"),
        r#"
        {
          "include": ["src/**/*.ts"],
          "exclude": ["generated/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("empty-exclude/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("useful-patterns/tsconfig.json"),
        r#"
        {
          "include": ["src/**/*.ts"],
          "exclude": ["src/generated/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("useful-patterns/src/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("useful-patterns/src/generated/skip.ts"),
        "export const skip = true;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:src/missing/**/*.ts",
            fixture
                .path()
                .join("empty-include/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"src/missing/**/*.ts\" matched 0 files under base dir {}.",
            fixture.path().join("empty-include").to_string_lossy()
        ),
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
        Some(fixture.path().join("empty-include/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:exclude:generated/**/*.ts",
            fixture
                .path()
                .join("empty-exclude/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"generated/**/*.ts\" removed 0 files from 1 included file(s) under base dir {}.",
            fixture.path().join("empty-exclude").to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(fixture.path().join("empty-exclude/tsconfig.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("useful-patterns/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "useful include/exclude patterns should not report tsconfig-pattern findings"
    );
}

#[test]
fn tsconfig_pattern_severity_contract_keeps_noop_excludes_non_blocking() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("empty-include/tsconfig.json"),
        r#"
        {
          "include": ["src/missing/**/*.mts"]
        }
        "#,
    );
    write(
        fixture.path().join("empty-include/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture
            .path()
            .join("noop-node-modules-exclude/tsconfig.json"),
        r#"
        {
          "include": ["src/**/*.ts"],
          "exclude": ["node_modules"]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("noop-node-modules-exclude/src/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("noop-generated-exclude/tsconfig.json"),
        r#"
        {
          "include": ["src/**/*.ts"],
          "exclude": ["generated/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("noop-generated-exclude/src/index.ts"),
        "export const ok = true;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:src/missing/**/*.mts",
            fixture
                .path()
                .join("empty-include/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"src/missing/**/*.mts\" matched 0 files under base dir {}.",
            fixture.path().join("empty-include").to_string_lossy()
        ),
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
        Some(fixture.path().join("empty-include/tsconfig.json")),
    );

    let noop_exclude_id = format!(
        "tsconfig-patterns:{}:exclude:node_modules",
        fixture
            .path()
            .join("noop-node-modules-exclude/tsconfig.json")
            .to_string_lossy()
    );
    assert!(
        outcome
            .findings
            .iter()
            .all(|finding| finding.id != noop_exclude_id),
        "default node_modules exclude should not report no-op audit noise"
    );

    let non_default_noop_exclude_id = format!(
        "tsconfig-patterns:{}:exclude:generated/**/*.ts",
        fixture
            .path()
            .join("noop-generated-exclude/tsconfig.json")
            .to_string_lossy()
    );
    assert_has_finding(
        &outcome.findings,
        &non_default_noop_exclude_id,
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"generated/**/*.ts\" removed 0 files from 1 included file(s) under base dir {}.",
            fixture
                .path()
                .join("noop-generated-exclude")
                .to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(
            fixture
                .path()
                .join("noop-generated-exclude/tsconfig.json"),
        ),
    );

    let non_default_noop_exclude_finding = outcome
        .findings
        .iter()
        .find(|finding| finding.id == non_default_noop_exclude_id)
        .expect("non-default no-op exclude finding should exist");
    let summary = summarize_findings(
        std::slice::from_ref(non_default_noop_exclude_finding),
        &outcome.fixes,
        &StructureReport {
            is_monorepo: false,
            package_count: 0,
            env_directories: 0,
            config_files: 1,
            recommendations: Vec::new(),
        },
    );

    assert_eq!(summary.status, "clean");
    assert_eq!(summary.blocking_findings, 0);
    assert_eq!(summary.warning_findings, 0);
    assert_eq!(summary.info_findings, 1);
}

#[test]
fn tsconfig_check_downgrades_missing_next_generated_types_include_for_next_packages() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let next_hint = "Next.js generates .next/types during development or build, so this include can be empty before .next exists.";
    let generic_hint =
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.";

    write(
        fixture.path().join("next-app/package.json"),
        r#"{ "devDependencies": { "next": "15.0.0" } }"#,
    );
    write(
        fixture.path().join("next-app/tsconfig.json"),
        r#"{ "include": ["./.next/types/**/*.ts"] }"#,
    );

    write(
        fixture.path().join("plain-app/package.json"),
        r#"{ "devDependencies": { "react": "19.0.0" } }"#,
    );
    write(
        fixture.path().join("plain-app/tsconfig.json"),
        r#"{ "include": [".next/types/**/*.ts"] }"#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");
    let next_tsconfig = fixture.path().join("next-app/tsconfig.json");
    let plain_tsconfig = fixture.path().join("plain-app/tsconfig.json");

    let next_id = format!(
        "tsconfig-patterns:{}:include:./.next/types/**/*.ts",
        next_tsconfig.to_string_lossy()
    );
    assert_has_finding(
        &outcome.findings,
        &next_id,
        Severity::Info,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"./.next/types/**/*.ts\" matched 0 files under base dir {}.",
            fixture.path().join("next-app").to_string_lossy()
        ),
        next_hint,
        Some(next_tsconfig.clone()),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:.next/types/**/*.ts",
            plain_tsconfig.to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \".next/types/**/*.ts\" matched 0 files under base dir {}.",
            fixture.path().join("plain-app").to_string_lossy()
        ),
        generic_hint,
        Some(plain_tsconfig),
    );

    let next_finding = outcome
        .findings
        .iter()
        .find(|finding| finding.id == next_id)
        .expect("Next generated types finding should exist");
    let summary = summarize_findings(
        std::slice::from_ref(next_finding),
        &outcome.fixes,
        &StructureReport {
            is_monorepo: false,
            package_count: 1,
            env_directories: 0,
            config_files: 1,
            recommendations: Vec::new(),
        },
    );

    assert_eq!(summary.status, "clean");
    assert_eq!(summary.blocking_findings, 0);
    assert_eq!(summary.warning_findings, 0);
    assert_eq!(summary.info_findings, 1);
}

#[test]
fn tsconfig_check_uses_default_include_and_allow_js_rules_for_pattern_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("exclude-only/tsconfig.json"),
        r#"
        {
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("exclude-only/src/index.d.ts"),
        "export declare const ok: true;\n",
    );

    write(
        fixture.path().join("js-without-allowjs/tsconfig.json"),
        r#"
        {
          "include": ["src"]
        }
        "#,
    );
    write(
        fixture.path().join("js-without-allowjs/src/index.js"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("js-with-allowjs/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "allowJs": true
          },
          "include": ["src"]
        }
        "#,
    );
    write(
        fixture.path().join("js-with-allowjs/src/index.js"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("question-mark-include/tsconfig.json"),
        r#"
        {
          "include": ["src/file?.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("question-mark-include/src/file1.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("star-zero-include/tsconfig.json"),
        r#"
        {
          "include": ["src/file*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("star-zero-include/src/file.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("inherited-allowjs/tsconfig.json"),
        r#"
        {
          "extends": "./tsconfig.base.json",
          "include": ["src"]
        }
        "#,
    );
    write(
        fixture.path().join("inherited-allowjs/tsconfig.base.json"),
        r#"
        {
          "compilerOptions": {
            "allowJs": true
          }
        }
        "#,
    );
    write(
        fixture.path().join("inherited-allowjs/src/index.js"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("inherited-outdir/tsconfig.json"),
        r#"
        {
          "extends": "./tsconfig.base.json",
          "exclude": ["dist/**/*.d.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("inherited-outdir/tsconfig.base.json"),
        r#"
        {
          "compilerOptions": {
            "outDir": "./dist"
          }
        }
        "#,
    );
    write(
        fixture.path().join("inherited-outdir/src/index.d.ts"),
        "export declare const source: true;\n",
    );
    write(
        fixture.path().join("inherited-outdir/dist/index.d.ts"),
        "export declare const built: true;\n",
    );
    write(
        fixture.path().join("outdir-include/tsconfig.json"),
        r#"
        {
          "extends": "./tsconfig.base.json",
          "include": ["dist"]
        }
        "#,
    );
    write(
        fixture.path().join("outdir-include/tsconfig.base.json"),
        r#"
        {
          "compilerOptions": {
            "outDir": "./dist"
          }
        }
        "#,
    );
    write(
        fixture.path().join("outdir-include/dist/index.d.ts"),
        "export declare const built: true;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:exclude:missing/**/*.ts",
            fixture
                .path()
                .join("exclude-only/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"missing/**/*.ts\" removed 0 files from 1 included file(s) under base dir {}.",
            fixture.path().join("exclude-only").to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(fixture.path().join("exclude-only/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:src",
            fixture
                .path()
                .join("js-without-allowjs/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"src\" matched 0 files under base dir {}.",
            fixture.path().join("js-without-allowjs").to_string_lossy()
        ),
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
        Some(fixture.path().join("js-without-allowjs/tsconfig.json")),
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("js-with-allowjs/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "allowJs-enabled include patterns should accept JavaScript inputs"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("question-mark-include/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "question-mark wildcards should match single-character file names"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("star-zero-include/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "star wildcards should still match zero-width suffixes"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("inherited-allowjs/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "allowJs inherited through extends should allow JavaScript inputs"
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:exclude:dist/**/*.d.ts",
            fixture
                .path()
                .join("inherited-outdir/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"dist/**/*.d.ts\" removed 0 files from 1 included file(s) under base dir {}.",
            fixture.path().join("inherited-outdir").to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(fixture.path().join("inherited-outdir/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:dist",
            fixture
                .path()
                .join("outdir-include/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"dist\" matched 0 files under base dir {}.",
            fixture.path().join("outdir-include").to_string_lossy()
        ),
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
        Some(fixture.path().join("outdir-include/tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_skips_explicitly_empty_inputs_and_implicit_default_excludes() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("include-empty/tsconfig.json"),
        r#"
        {
          "include": [],
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("include-empty/src/index.d.ts"),
        "export declare const ok: true;\n",
    );

    write(
        fixture.path().join("files-empty/tsconfig.json"),
        r#"
        {
          "files": [],
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("files-empty/src/index.d.ts"),
        "export declare const ok: true;\n",
    );

    write(
        fixture.path().join("default-excludes/tsconfig.json"),
        r#"
        {
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("default-excludes/node_modules/pkg/index.d.ts"),
        "export declare const ignored: true;\n",
    );
    write(
        fixture.path().join("files-with-exclude/tsconfig.json"),
        r#"
        {
          "files": ["src/index.d.ts"],
          "exclude": ["src/**/*.d.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("files-with-exclude/src/index.d.ts"),
        "export declare const explicit: true;\n",
    );
    write(
        fixture.path().join("duplicate-excludes/tsconfig.json"),
        r#"
        {
          "include": ["src/**/*.d.ts"],
          "exclude": ["src/generated/**/*.d.ts", "src/generated/**/*.d.ts"]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("duplicate-excludes/src/generated/index.d.ts"),
        "export declare const duplicated: true;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("include-empty/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "explicitly empty include arrays should not fall back to default include scanning"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("files-empty/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "files arrays should disable default include scanning even when they are empty"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("default-excludes/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "implicit default excludes like node_modules should not count as included inputs"
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:exclude:src/**/*.d.ts",
            fixture
                .path()
                .join("files-with-exclude/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"src/**/*.d.ts\" removed 0 files from 1 included file(s) under base dir {}.",
            fixture.path().join("files-with-exclude").to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(fixture.path().join("files-with-exclude/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:exclude:src/generated/**/*.d.ts",
            fixture
                .path()
                .join("duplicate-excludes/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Info,
        "Exclude pattern does not filter any included files",
        &format!(
            "exclude pattern \"src/generated/**/*.d.ts\" removed 0 files from 0 included file(s) under base dir {}.",
            fixture.path().join("duplicate-excludes").to_string_lossy()
        ),
        "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
        Some(fixture.path().join("duplicate-excludes/tsconfig.json")),
    );
}

#[test]
fn tsconfig_check_reports_output_path_overlap_contracts() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("outdir-src/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": "./src",
            "outDir": "./src"
          }
        }
        "#,
    );
    write(
        fixture.path().join("outdir-src/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("outdir-src-generated/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": "./src",
            "outDir": "./src/generated"
          }
        }
        "#,
    );
    write(
        fixture.path().join("outdir-src-generated/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("outdir-dist/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": "./src",
            "outDir": "./dist"
          }
        }
        "#,
    );
    write(
        fixture.path().join("outdir-dist/src/index.ts"),
        "export const ok = true;\n",
    );

    write(
        fixture.path().join("outdir-contains-source/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": "./src/generated",
            "outDir": "./src"
          },
          "files": []
        }
        "#,
    );

    write(
        fixture.path().join("rootdir-dot/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": ".",
            "outDir": "./src"
          },
          "files": ["index.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("rootdir-dot/index.ts"),
        "export const root = true;\n",
    );

    write(
        fixture
            .path()
            .join("files-with-unmatched-include/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "outDir": "./src/generated"
          },
          "files": ["index.ts"],
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("files-with-unmatched-include/index.ts"),
        "export const root = true;\n",
    );

    write(
        fixture.path().join("outdir-dot/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "outDir": "."
          }
        }
        "#,
    );
    write(
        fixture.path().join("outdir-dot/src/index.ts"),
        "export const source = true;\n",
    );

    write(
        fixture.path().join("mixed-inputs/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "outDir": "./src"
          },
          "files": ["src/index.ts", "config.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("mixed-inputs/src/index.ts"),
        "export const emitted = true;\n",
    );
    write(
        fixture.path().join("mixed-inputs/config.ts"),
        "export const config = true;\n",
    );

    write(
        fixture.path().join("windows-style/tsconfig.json"),
        r#"
        {
          "compilerOptions": {
            "rootDir": ".\\src",
            "outDir": ".\\src\\generated"
          }
        }
        "#,
    );
    write(
        fixture.path().join("windows-style/src/index.ts"),
        "export const ok = true;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-equals-source:outdir-src/src",
            fixture
                .path()
                .join("outdir-src/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory overlaps the TypeScript source root",
        "outDir \"outdir-src/src\" overlaps source root \"outdir-src/src\".",
        "Move emit output outside the source root so build artifacts do not overwrite source files.",
        Some(fixture.path().join("outdir-src/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-nested-in-source:outdir-src-generated/src/generated",
            fixture
                .path()
                .join("outdir-src-generated/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory is nested inside the TypeScript source root",
        "outDir \"outdir-src-generated/src/generated\" is nested inside source root \"outdir-src-generated/src\".",
        "Move emit output outside the source root so build artifacts do not overwrite source files.",
        Some(fixture.path().join("outdir-src-generated/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-contains-source:outdir-contains-source/src",
            fixture
                .path()
                .join("outdir-contains-source/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Output directory contains the TypeScript source root",
        "outDir \"outdir-contains-source/src\" contains source root \"outdir-contains-source/src/generated\".",
        "Prefer an output directory that is completely separate from the TypeScript source root.",
        Some(fixture.path().join("outdir-contains-source/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-nested-in-source:rootdir-dot/src",
            fixture
                .path()
                .join("rootdir-dot/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory is nested inside the TypeScript source root",
        "outDir \"rootdir-dot/src\" is nested inside source root \"rootdir-dot\".",
        "Move emit output outside the source root so build artifacts do not overwrite source files.",
        Some(fixture.path().join("rootdir-dot/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-contains-input:outdir-dot",
            fixture
                .path()
                .join("outdir-dot/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory contains TypeScript input files",
        "outDir \"outdir-dot\" contains TypeScript input \"outdir-dot/src/index.ts\".",
        "Move emit output outside any directory that currently contains TypeScript input files.",
        Some(fixture.path().join("outdir-dot/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-equals-source:mixed-inputs/src",
            fixture
                .path()
                .join("mixed-inputs/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory overlaps the TypeScript source root",
        "outDir \"mixed-inputs/src\" overlaps source root \"mixed-inputs/src\".",
        "Move emit output outside the source root so build artifacts do not overwrite source files.",
        Some(fixture.path().join("mixed-inputs/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-output-paths:{}:outdir-nested-in-source:windows-style/src/generated",
            fixture
                .path()
                .join("windows-style/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Output directory is nested inside the TypeScript source root",
        "outDir \"windows-style/src/generated\" is nested inside source root \"windows-style/src\".",
        "Move emit output outside the source root so build artifacts do not overwrite source files.",
        Some(fixture.path().join("windows-style/tsconfig.json")),
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-output-paths:{}:",
                fixture
                    .path()
                    .join("outdir-dist/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "safe dist output directories should not report overlap findings"
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-output-paths:{}:",
                fixture
                    .path()
                    .join("files-with-unmatched-include/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "unmatched include patterns should not create output overlap findings when only files inputs are active"
    );
}

#[test]
fn tsconfig_check_inherits_pattern_fields_and_reports_invalid_pattern_entries() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("shared/tsconfig.base.json"),
        r#"
        {
          "include": ["./src/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("app-inherited-include/tsconfig.json"),
        r#"
        {
          "extends": "../shared/tsconfig.base.json"
        }
        "#,
    );
    write(
        fixture.path().join("app-inherited-include/src/index.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("app-missing-extends/tsconfig.json"),
        r#"
        {
          "extends": "../shared-missing/tsconfig.base.json"
        }
        "#,
    );

    write(
        fixture.path().join("shared-empty/tsconfig.base.json"),
        r#"
        {
          "files": []
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("app-inherited-files-empty/tsconfig.json"),
        r#"
        {
          "extends": "../shared-empty/tsconfig.base.json",
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("app-inherited-files-empty/src/index.d.ts"),
        "export declare const ok: true;\n",
    );

    write(
        fixture.path().join("invalid-pattern-entry/tsconfig.json"),
        r#"
        {
          "include": [42]
        }
        "#,
    );
    write(
        fixture.path().join("invalid-files-entry/tsconfig.json"),
        r#"
        {
          "files": ["src/*.ts"],
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture.path().join("invalid-files-entry/src/index.d.ts"),
        "export declare const ok: true;\n",
    );
    write(
        fixture.path().join("invalid-files-directory/tsconfig.json"),
        r#"
        {
          "files": ["src"],
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );
    write(
        fixture
            .path()
            .join("invalid-files-directory/src/index.d.ts"),
        "export declare const ok: true;\n",
    );
    write(
        fixture.path().join("invalid-files-missing/tsconfig.json"),
        r#"
        {
          "files": ["src/missing.ts"],
          "exclude": ["missing/**/*.ts"]
        }
        "#,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_tsconfig_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns:{}:include:./src/**/*.ts",
            fixture
                .path()
                .join("app-inherited-include/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Warn,
        "Include pattern does not match any files",
        &format!(
            "include pattern \"./src/**/*.ts\" matched 0 files under base dir {}.",
            fixture.path().join("shared").to_string_lossy()
        ),
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
        Some(fixture.path().join("app-inherited-include/tsconfig.json")),
    );
    assert!(
        outcome.findings.iter().all(|finding| {
            !finding.id.starts_with(&format!(
                "tsconfig-patterns:{}:",
                fixture
                    .path()
                    .join("app-inherited-files-empty/tsconfig.json")
                    .to_string_lossy()
            ))
        }),
        "inherited files arrays should disable default include scanning on child configs"
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns-shape:{}:extends:missing:{}",
            fixture
                .path()
                .join("app-missing-extends/tsconfig.json")
                .to_string_lossy(),
            fixture
                .path()
                .join("app-missing-extends/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Inherited tsconfig could not be found",
        &format!(
            "{} extends ../shared-missing/tsconfig.base.json, but that config file could not be resolved.",
            fixture
                .path()
                .join("app-missing-extends/tsconfig.json")
                .to_string_lossy()
        ),
        "Fix missing extends targets before relying on inherited tsconfig pattern settings.",
        Some(fixture.path().join("app-missing-extends/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns-shape:{}:include:entry-0:{}",
            fixture
                .path()
                .join("invalid-pattern-entry/tsconfig.json")
                .to_string_lossy(),
            fixture
                .path()
                .join("invalid-pattern-entry/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "\"include\" contains a non-string pattern",
        &format!(
            "{} declares include[0], but TypeScript expects string patterns.",
            fixture
                .path()
                .join("invalid-pattern-entry/tsconfig.json")
                .to_string_lossy()
        ),
        "Rewrite include as an array of string globs before relying on TypeScript input discovery.",
        Some(fixture.path().join("invalid-pattern-entry/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns-shape:{}:files:entry-0-wildcard:{}",
            fixture
                .path()
                .join("invalid-files-entry/tsconfig.json")
                .to_string_lossy(),
            fixture
                .path()
                .join("invalid-files-entry/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "\"files\" entries must point to explicit files",
        &format!(
            "{} declares files[0] as src/*.ts, but TypeScript files entries cannot use glob wildcards.",
            fixture
                .path()
                .join("invalid-files-entry/tsconfig.json")
                .to_string_lossy()
        ),
        "Rewrite files as an array of string paths before relying on explicit TypeScript inputs.",
        Some(fixture.path().join("invalid-files-entry/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns-shape:{}:files:entry-0-directory:{}",
            fixture
                .path()
                .join("invalid-files-directory/tsconfig.json")
                .to_string_lossy(),
            fixture
                .path()
                .join("invalid-files-directory/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "\"files\" entries must point to files",
        &format!(
            "{} declares files[0] as src, but that path resolves to a directory.",
            fixture
                .path()
                .join("invalid-files-directory/tsconfig.json")
                .to_string_lossy()
        ),
        "Rewrite files as an array of string paths before relying on explicit TypeScript inputs.",
        Some(fixture.path().join("invalid-files-directory/tsconfig.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "tsconfig-patterns-shape:{}:files:entry-0-missing:{}",
            fixture
                .path()
                .join("invalid-files-missing/tsconfig.json")
                .to_string_lossy(),
            fixture
                .path()
                .join("invalid-files-missing/tsconfig.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "\"files\" entries must point to existing files",
        &format!(
            "{} declares files[0] as src/missing.ts, but that path does not resolve to an existing file.",
            fixture
                .path()
                .join("invalid-files-missing/tsconfig.json")
                .to_string_lossy()
        ),
        "Rewrite files as an array of string paths before relying on explicit TypeScript inputs.",
        Some(fixture.path().join("invalid-files-missing/tsconfig.json")),
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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
    assert_eq!(
        finding.file,
        Some(fixture.path().join("root/tsconfig.json"))
    );
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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
    assert_eq!(
        finding.file,
        Some(fixture.path().join("root/tsconfig.json"))
    );
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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
        parse_finding.detail.contains("pkg-a/tsconfig.json"),
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
            fixture
                .path()
                .join("entries/tsconfig.json")
                .to_string_lossy()
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
            fixture
                .path()
                .join("entries/tsconfig.json")
                .to_string_lossy()
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
            fixture
                .path()
                .join("entries/tsconfig.json")
                .to_string_lossy()
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
    write(
        &target_path,
        r#"{ "compilerOptions": { "composite": true } }"#,
    );

    let original_permissions = fs::metadata(&target_path)
        .expect("target metadata should exist")
        .permissions();
    let mut unreadable_permissions = original_permissions.clone();
    unreadable_permissions.set_mode(0o000);
    fs::set_permissions(&target_path, unreadable_permissions)
        .expect("target permissions should update");

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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

    let project =
        discover_project(fixture.path().join("root").as_path()).expect("project should discover");
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
