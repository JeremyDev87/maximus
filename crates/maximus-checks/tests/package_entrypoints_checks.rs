use std::fs;
use std::path::{Path, PathBuf};

use maximus_checks::package_entrypoints::run_package_entrypoints_check;
use maximus_core::{discover_project, Severity};
use tempfile::TempDir;

#[test]
fn package_entrypoints_check_reports_missing_relative_targets_and_recurses_through_branches() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/missing-main.js",
          "module": "./dist/module",
          "types": "./dist/types",
          "bin": {
            "maximus": "./bin/cli",
            "missing": "./bin/missing.js"
          },
          "exports": {
            ".": "./dist/index.js",
            "./feature": [
              { "default": "./dist/feature.js" },
              "./dist/fallback.js"
            ],
            "./nested": {
              "import": "./dist/import.js",
              "default": "./dist/missing-exports.js"
            },
            "./package": "./dist/package.js",
            "./url": "./dist/url.js"
          },
          "imports": {
            "#ok": "./src/ok.ts",
            "#missing": ["react", "./src/missing.ts"],
            "#nested": {
              "types": "./src/types.d.ts",
              "default": "./src/missing-imports.js"
            }
          }
        }
        "##,
    );
    write(fixture.path().join("dist/module.js"), "export default 1;\n");
    write(
        fixture.path().join("dist/types.d.ts"),
        "export type Types = string;\n",
    );
    write(fixture.path().join("bin/cli.js"), "#!/usr/bin/env node\n");
    write(fixture.path().join("dist/index.js"), "export default 1;\n");
    write(
        fixture.path().join("dist/feature.js"),
        "export default 1;\n",
    );
    write(fixture.path().join("dist/import.js"), "export default 1;\n");
    write(
        fixture.path().join("dist/package.js"),
        "export default 1;\n",
    );
    write(fixture.path().join("dist/url.js"), "export default 1;\n");
    write(
        fixture.path().join("src/ok.ts"),
        "export const ok = true;\n",
    );
    write(
        fixture.path().join("src/types.d.ts"),
        "export type Types = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/missing-main.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/missing-main.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/missing", "./bin/missing.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json bin/missing points to ./bin/missing.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./nested/default", "./dist/missing-exports.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json exports/./nested/default points to ./dist/missing-exports.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#nested/default", "./src/missing-imports.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json imports/#nested/default points to ./src/missing-imports.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );

    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "module", "./dist/module"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "./dist/types"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/maximus", "./bin/cli"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "./dist/index.js"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(
            fixture.path(),
            "exports/./feature/[1]",
            "./dist/fallback.js",
        ),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./package", "./dist/package.js"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./url", "./dist/url.js"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#missing/[0]", "react"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#missing/[1]", "./src/missing.ts"),
    );
}

#[test]
fn package_entrypoints_check_treats_main_like_bare_targets_as_package_relative_paths() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "index",
          "module": "dist/module",
          "types": "types/index",
          "bin": {
            "cli": "bin/cli"
          },
          "exports": {
            ".": "./index.js",
            "./nested": { "default": "./dist/module.js" }
          },
          "imports": {
            "#alias": ["@scope/pkg", "https://example.com/x.js"]
          }
        }
        "##,
    );
    write(fixture.path().join("index.js"), "export default 1;\n");
    write(fixture.path().join("dist/module.js"), "export default 1;\n");
    write(
        fixture.path().join("types/index.d.ts"),
        "export type Maximus = string;\n",
    );
    write(fixture.path().join("bin/cli.js"), "#!/usr/bin/env node\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_reports_single_segment_file_paths() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "index.js",
          "bin": {
            "cli": "cli.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "index.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to index.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/cli", "cli.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json bin/cli points to cli.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_bin_directory_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "bin": {
            "cli": "./bin"
          }
        }
        "##,
    );
    write(fixture.path().join("bin/index.js"), "#!/usr/bin/env node\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/cli", "./bin"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json bin/cli points to ./bin, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_wildcards_for_main_like_fields() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/*.js"
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/*.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ./dist/*.js, but main/module/types/bin targets must not use wildcard patterns.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_directory_targets_when_the_entrypoint_has_an_explicit_extension(
) {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "main": "./dist/module.js" }"#,
    );
    write(
        fixture.path().join("dist/module.js/index.js"),
        "console.log('nested');",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "package-entrypoints:{}:main:./dist/module.js",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/module.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_accepts_dotted_directory_targets_without_known_extensions() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "main": "./dist/v1.0" }"#,
    );
    write(
        fixture.path().join("dist/v1.0/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/v1.0"),
    );
}

#[test]
fn package_entrypoints_check_reports_dependency_named_targets_for_main_like_fields() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "dependencies": {
            "react": "^19.0.0",
            "@scope/pkg": "^1.0.0",
            "@scope/cli": "^1.0.0"
          },
          "main": "react",
          "module": "node:fs",
          "types": "@scope/pkg",
          "bin": {
            "cli": "@scope/cli"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "react"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to react, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "@scope/pkg"),
        Severity::Warn,
        "Package entrypoint target does not exist",
        "package.json types points to @scope/pkg, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/cli", "@scope/cli"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json bin/cli points to @scope/cli, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "module", "node:fs"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json module points to node:fs, but main/module/types/bin targets must be package-local paths and cannot use URL-like schemes.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_url_like_schemes_for_main_like_fields() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "ftp:shim.js"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "ftp:shim.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ftp:shim.js, but main/module/types/bin targets must be package-local paths and cannot use URL-like schemes.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_multi_segment_package_relative_paths() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "components/Button.js"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "components/Button.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to components/Button.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_missing_single_segment_bare_local_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "server",
          "types": "entry"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "server"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to server, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "entry"),
        Severity::Warn,
        "Package entrypoint target does not exist",
        "package.json types points to entry, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_exports_and_imports_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "./dist/../dist/main.js",
            "./bad": "./node_modules/foo.js"
          },
          "imports": {
            "#bad": "../outside.js"
          }
        }
        "##,
    );
    write(fixture.path().join("dist/main.js"), "export default 1;\n");
    write(
        fixture.path().join("node_modules/foo.js"),
        "export default 'bad';\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "./dist/../dist/main.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to ./dist/../dist/main.js, but exports targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./bad", "./node_modules/foo.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/./bad points to ./node_modules/foo.js, but exports targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#bad", "../outside.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#bad points to ../outside.js, but imports local targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_invalid_non_local_import_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#dot": ".foo",
            "#ftp": "ftp:shim.js",
            "#node": "node:fs",
            "#pkg": "@scope/pkg/subpath",
            "#traversal": "pkg/../sub.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#dot", ".foo"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#dot points to .foo, but imports targets must be package-local paths under ./ or valid external package specifiers.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#ftp", "ftp:shim.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#ftp points to ftp:shim.js, but imports targets must be package-local paths under ./ or valid external package specifiers.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#node", "node:fs"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#node points to node:fs, but imports targets must be package-local paths under ./ or valid external package specifiers.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#pkg", "@scope/pkg/subpath"),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#traversal", "pkg/../sub.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#traversal points to pkg/../sub.js, but imports targets must be package-local paths under ./ or valid external package specifiers.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_non_string_entrypoint_values() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": true,
          "types": 123,
          "bin": {
            "cli": null
          },
          "exports": false,
          "imports": {
            "#alias": 0
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "main", "boolean"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main must be a string target or nested fallback branches, but found boolean.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "types", "number"),
        Severity::Warn,
        "Package entrypoint target is invalid",
        "package.json types must be a string target or nested fallback branches, but found number.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "bin/cli", "null"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json bin/cli must be a string target or nested fallback branches, but found null.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "exports", "boolean"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports must be a string target or nested fallback branches, but found boolean.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "imports/#alias", "number"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#alias must be a string target or nested fallback branches, but found number.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_compound_values_for_main_like_fields_and_nested_bin_values() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": ["./dist/index.js"],
          "module": {
            "default": "./dist/module.js"
          },
          "types": {
            "default": "./dist/types.d.ts"
          },
          "bin": {
            "cli": {
              "default": "./bin/cli.js"
            }
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.js"),
        "module.exports = {};\n",
    );
    write(
        fixture.path().join("dist/module.js"),
        "module.exports = {};\n",
    );
    write(
        fixture.path().join("dist/types.d.ts"),
        "export type T = string;\n",
    );
    write(fixture.path().join("bin/cli.js"), "#!/usr/bin/env node\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "main", "array"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main must be a string target or nested fallback branches, but found array.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "module", "object"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json module must be a string target or nested fallback branches, but found object.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "types", "object"),
        Severity::Warn,
        "Package entrypoint target is invalid",
        "package.json types must be a string target or nested fallback branches, but found object.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "bin/cli", "object"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json bin/cli must be a string target or nested fallback branches, but found object.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_allows_null_targets_for_exports_and_imports() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./private/*": null
          },
          "imports": {
            "#private/*": null
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "exports/./private/*", "null"),
    );
    assert_no_finding(
        &outcome.findings,
        &invalid_type_finding_id(fixture.path(), "imports/#private/*", "null"),
    );
}

#[test]
fn package_entrypoints_check_rejects_percent_encoded_invalid_segments() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./node_modules%2ffoo.js",
          "exports": {
            ".": "./node_modules%2ffoo.js"
          },
          "imports": {
            "#bad": "./node_modules%2ffoo.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./node_modules%2ffoo.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ./node_modules%2ffoo.js, but main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "./node_modules%2ffoo.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to ./node_modules%2ffoo.js, but exports targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#bad", "./node_modules%2ffoo.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#bad points to ./node_modules%2ffoo.js, but imports local targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_empty_target_path_segments() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist//index.js",
          "exports": {
            ".": "./dist/%2findex.js"
          },
          "imports": {
            "#bad": "./dist//index.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist//index.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ./dist//index.js, but main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "./dist/%2findex.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to ./dist/%2findex.js, but exports targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#bad", "./dist//index.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#bad points to ./dist//index.js, but imports local targets cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_does_not_recurse_on_root_directory_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "."
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "."),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ., but main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
#[cfg(unix)]
fn package_entrypoints_check_limits_symlink_cycles_in_nested_package_manifests() {
    use std::os::unix::fs as unix_fs;

    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./pkg"
        }
        "##,
    );
    write(
        fixture.path().join("pkg/package.json"),
        r##"
        {
          "main": "./loop"
        }
        "##,
    );
    unix_fs::symlink(fixture.path().join("pkg"), fixture.path().join("pkg/loop"))
        .expect("symlink cycle should be created");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(
        outcome.findings.len() <= 12,
        "cycle guard should cap nested manifest findings, got {}",
        outcome.findings.len()
    );
}

#[test]
fn package_entrypoints_check_reports_empty_targets_as_invalid() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "",
          "imports": {
            "#empty": ""
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", ""),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to , but main/module/types/bin targets must not be empty.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#empty", ""),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#empty points to , but imports targets must not be empty.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_compresses_multiple_broken_branches_per_key() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": ["./dist/missing-one.js", "./dist/missing-two.js"]
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(
        outcome.findings[0].id,
        finding_id(fixture.path(), "exports/./[0]", "./dist/missing-one.js")
    );
}

#[test]
fn package_entrypoints_check_compresses_root_exports_arrays() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": ["./dist/missing-one.js", "./dist/missing-two.js"]
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(
        outcome.findings[0].id,
        finding_id(fixture.path(), "exports/[0]", "./dist/missing-one.js")
    );
}

#[test]
fn package_entrypoints_check_compresses_multiple_broken_object_branches_even_with_one_valid_branch()
{
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": {
              "import": "./dist/import.js",
              "types": "./dist/missing-types.d.ts",
              "default": "./dist/missing-default.js"
            }
          }
        }
        "##,
    );
    write(fixture.path().join("dist/import.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_eq!(outcome.findings.len(), 1);
}

#[test]
fn package_entrypoints_check_compresses_root_exports_condition_objects() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "import": "./dist/missing-import.js",
            "require": "./dist/missing-require.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_eq!(outcome.findings.len(), 1);
}

#[test]
fn package_entrypoints_check_prefers_error_over_warn_when_compressing_branches() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": {
              "types": "./dist/missing-types.d.ts",
              "default": "./dist/missing-default.js"
            }
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].severity, Severity::Error);
}

#[test]
fn package_entrypoints_check_reports_invalid_exports_but_skips_external_imports() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": ["left-pad", "file:///tmp/asset.js"],
            "./nested": { "default": "#internal" }
          },
          "imports": {
            "#alias": ["@scope/pkg", "https://example.com/x.js"]
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./[0]", "left-pad"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/./[0] points to left-pad, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./nested/default", "#internal"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/./nested/default points to #internal, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#alias/[0]", "@scope/pkg"),
    );
}

#[test]
fn package_entrypoints_check_skips_absolute_targets_for_exports_and_imports() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "/usr/local/lib/maximus.js"
          },
          "imports": {
            "#abs": "/usr/local/lib/maximus.js",
            "#winabs": "C:/maximus/shim.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "/usr/local/lib/maximus.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to /usr/local/lib/maximus.js, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#abs", "/usr/local/lib/maximus.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#abs points to /usr/local/lib/maximus.js, but imports local targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#winabs", "C:/maximus/shim.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json imports/#winabs points to C:/maximus/shim.js, but imports local targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[cfg(unix)]
#[test]
fn package_entrypoints_check_accepts_posix_colon_filenames() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "foo:bar.js"
        }
        "##,
    );
    write(fixture.path().join("foo:bar.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_accepts_windows_style_relative_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": ".\\dist\\index.js"
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", ".\\dist\\index.js"),
    );
}

#[test]
fn package_entrypoints_check_accepts_package_relative_main_and_bin_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "dist/index.js",
          "bin": {
            "maximus": "bin/cli.js"
          }
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");
    write(fixture.path().join("bin/cli.js"), "#!/usr/bin/env node\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_prefers_file_extension_matches_before_directory_fallback() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/foo"
        }
        "##,
    );
    fs::create_dir_all(fixture.path().join("dist/foo")).expect("directory should exist");
    write(fixture.path().join("dist/foo.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_rejects_targets_that_escape_the_package_root() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let outside_file = fixture.path().parent().unwrap().join("outside-shared.js");
    write(&outside_file, "export default 1;\n");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "../outside-shared.js",
          "exports": {
            ".": "../outside-shared.js"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "../outside-shared.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ../outside-shared.js, but main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "../outside-shared.js"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to ../outside-shared.js, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_requires_concrete_file_matches_for_directories_and_wildcards() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module",
          "exports": {
            "./*": "./dist/*"
          }
        }
        "##,
    );
    fs::create_dir_all(fixture.path().join("dist/module"))
        .expect("directory-only entrypoint should exist");
    fs::create_dir_all(fixture.path().join("dist")).expect("dist dir should exist");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/module, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./*", "./dist/*"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json exports/./* points to ./dist/*, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_falls_back_to_index_when_nested_package_main_is_broken() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./missing.js"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_preserves_nested_incompatible_targets_when_index_fallback_exists() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./index.d.ts"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/index.js"),
        "module.exports = {};\n",
    );
    write(
        fixture.path().join("dist/module/index.d.ts"),
        "export type Bad = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "main",
            "./index.d.ts",
        ),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json main points to ./index.d.ts, but runtime entrypoints must not point to declaration-only files such as .d.ts.",
        "Point main/module/bin to a runtime file instead of a declaration-only file.",
        Some(fixture.path().join("dist/module/package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
    );
}

#[test]
fn package_entrypoints_check_preserves_nested_structural_findings_when_index_fallback_exists() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./missing.js",
          "exports": {
            "#bad": "./index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/index.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "exports/#bad",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/#bad uses an invalid key. exports keys must not start with #.",
        "Rename the exports entry to . or ./subpath, or use a condition key without a # prefix.",
        Some(fixture.path().join("dist/module/package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
    );
}

#[test]
fn package_entrypoints_check_respects_explicit_wildcard_extensions() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./*": "./dist/*.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/feature.js.cjs"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./*", "./dist/*.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json exports/./* points to ./dist/*.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_treats_arrays_as_fallback_when_an_earlier_relative_branch_exists() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": ["./dist/index.js", "./dist/missing.js"]
          }
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_skips_invalid_specifier_array_branches_after_valid_fallbacks() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": ["not:valid", "./dist/index.js"]
          }
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./[0]", "not:valid"),
    );
    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_treats_condition_objects_inside_arrays_as_fallback_branches() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": [
              {
                "default": "./dist/index.js",
                "types": "./dist/missing-types.d.ts"
              },
              "./dist/missing-fallback.js"
            ]
          }
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_preserves_structural_findings_before_valid_array_fallbacks() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": [
              {
                "#bad": "./dist/index.js"
              },
              "./dist/index.js"
            ]
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/./[0]/#bad"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/./[0]/#bad uses an invalid key. exports keys must not start with #.",
        "Rename the exports entry to . or ./subpath, or use a condition key without a # prefix.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./[1]", "./dist/index.js"),
    );
}

#[test]
fn package_entrypoints_check_accepts_directory_targets_with_nested_package_main() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./entry.js"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/entry.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_reports_other_nested_entrypoints_in_hidden_package_manifests() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./entry.js",
          "module": "./missing.js"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/entry.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "module",
            "./missing.js",
        ),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json module points to ./missing.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("dist/module/package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_nested_wildcard_incompatible_matches() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "./dist/module"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "exports": {
            "./*": "./dist/*"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/dist/index.d.ts"),
        "export type Nested = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "package-entrypoints:{}:exports/./*:./dist/*:./dist/index.d.ts",
            fixture
                .path()
                .join("dist/module/package.json")
                .to_string_lossy()
        ),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json exports/./* points to ./dist/*, but wildcard match ./dist/index.d.ts is a declaration-only file.",
        "Point runtime entrypoints to runtime files instead of declaration-only files.",
        Some(fixture.path().join("dist/module/package.json")),
    );
}

#[test]
fn package_entrypoints_check_treats_nested_import_arrays_with_valid_external_targets_as_resolved() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#nested": "./dist/module"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "imports": {
            "#nested": ["react", "./missing.js"]
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_no_finding(
        &outcome.findings,
        &finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "imports/#nested/[1]",
            "./missing.js",
        ),
    );
}

#[test]
fn package_entrypoints_check_treats_nested_condition_objects_as_valid_when_a_branch_resolves() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": {
            "default": "./entry.js",
            "types": "./missing-types.d.ts"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/entry.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "main",
            "object",
        ),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main must be a string target or nested fallback branches, but found object.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("dist/module/package.json")),
    );
}

#[test]
fn package_entrypoints_check_treats_nested_arrays_with_valid_condition_branches_as_resolved() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": [
            {
              "default": "./entry.js",
              "types": "./missing-types.d.ts"
            },
            "./missing-fallback.js"
          ]
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/entry.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &invalid_type_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "main",
            "array",
        ),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main must be a string target or nested fallback branches, but found array.",
        "Use a string target or nested fallback branches composed of strings, arrays, and objects.",
        Some(fixture.path().join("dist/module/package.json")),
    );
}

#[test]
fn package_entrypoints_check_preserves_types_branch_context_in_nested_exports() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "./dist/module"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "exports": {
            ".": {
              "types": "./index"
            }
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/index.d.ts"),
        "export type Maximus = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_reports_nested_types_targets_as_warn() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "types": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "types": "./missing"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "./dist/module"),
        Severity::Warn,
        "Package entrypoint target does not exist",
        "package.json types points to ./dist/module, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_missing_nested_package_paths_with_multiple_segments() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "packages/ui/index.js"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "packages/ui/index.js"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to packages/ui/index.js, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_accepts_native_addon_entrypoints() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./addon"
        }
        "##,
    );
    write(
        fixture.path().join("addon.node"),
        "native addon placeholder\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert!(outcome.findings.is_empty());
}

#[test]
fn package_entrypoints_check_reports_types_targets_as_warn() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "types": "./dist/missing-types.d.ts"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "./dist/missing-types.d.ts"),
        Severity::Warn,
        "Package entrypoint target does not exist",
        "package.json types points to ./dist/missing-types.d.ts, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_treats_bin_command_named_types_as_runtime_entrypoint() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "bin": {
            "types": "./bin/cli.d.ts"
          }
        }
        "##,
    );
    write(
        fixture.path().join("bin/cli.d.ts"),
        "export type Cli = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "bin/types", "./bin/cli.d.ts"),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json bin/types points to ./bin/cli.d.ts, but runtime entrypoints must not point to declaration-only files such as .d.ts.",
        "Point main/module/bin to a runtime file instead of a declaration-only file.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_uses_field_specific_extension_probing() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/index",
          "types": "./dist/types"
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.d.ts"),
        "export type Runtime = string;\n",
    );
    write(
        fixture.path().join("dist/types.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/index"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/index, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "./dist/types"),
        Severity::Warn,
        "Package entrypoint target does not exist",
        "package.json types points to ./dist/types, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_exports_but_skips_external_import_subpaths() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "pkg/subpath",
            "./feature": "@scope/pkg/subpath"
          },
          "imports": {
            "#pkg": "pkg/subpath",
            "#scoped": "@scope/pkg/subpath"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/.", "pkg/subpath"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/. points to pkg/subpath, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./feature", "@scope/pkg/subpath"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json exports/./feature points to @scope/pkg/subpath, but exports targets must stay within the package and start with ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#pkg", "pkg/subpath"),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#scoped", "@scope/pkg/subpath"),
    );
}

#[test]
fn package_entrypoints_check_rejects_parent_segments_in_main_like_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("packages/app/package.json"),
        r##"
        {
          "main": "../app/index.js"
        }
        "##,
    );
    write(
        fixture.path().join("packages/app/index.js"),
        "module.exports = {}\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id_for_package(
            &fixture.path().join("packages/app/package.json"),
            "main",
            "../app/index.js",
        ),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ../app/index.js, but main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("packages/app/package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_root_directory_alias_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./"
        }
        "##,
    );
    write(fixture.path().join("index.js"), "module.exports = {}\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./"),
        Severity::Error,
        "Package entrypoint target is invalid",
        "package.json main points to ./, but main/module/types/bin targets must point to a file or directory under ./.",
        "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_incompatible_exact_file_types() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/index.d.ts",
          "types": "./dist/index.js"
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.d.ts"),
        "export type Maximus = string;\n",
    );
    write(fixture.path().join("dist/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/index.d.ts"),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json main points to ./dist/index.d.ts, but runtime entrypoints must not point to declaration-only files such as .d.ts.",
        "Point main/module/bin to a runtime file instead of a declaration-only file.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "types", "./dist/index.js"),
        Severity::Warn,
        "Package entrypoint target uses an incompatible file type",
        "package.json types points to ./dist/index.js, but types targets must point to declaration files such as .d.ts, .d.mts, or .d.cts.",
        "Point types to a generated declaration file before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_root_import_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "react": "./src/index.js",
            "#ok": "./src/ok.js"
          }
        }
        "##,
    );
    write(fixture.path().join("src/index.js"), "export default 1;\n");
    write(fixture.path().join("src/ok.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/react"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#ok", "./src/ok.js"),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_import_subpath_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#alias/../x": "./src/index.js",
            "#ok/path": "./src/ok.js",
            "#/ok": "./src/slash.js"
          }
        }
        "##,
    );
    write(fixture.path().join("src/index.js"), "export default 1;\n");
    write(fixture.path().join("src/ok.js"), "export default 1;\n");
    write(fixture.path().join("src/slash.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/#alias/../x"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/#alias/../x uses an invalid key. imports keys must not contain empty, ., .., or node_modules path segments.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/#ok/path"),
    );
    assert_no_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/#/ok"),
    );
}

#[test]
fn package_entrypoints_check_reports_trailing_slash_import_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#alias/": "./src/index.js"
          }
        }
        "##,
    );
    write(fixture.path().join("src/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/#alias/"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/#alias/ uses an invalid key. imports keys must not contain empty, ., .., or node_modules path segments.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_exports_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "#alias": "./dist/index.js",
            ".": {
              "#nested": "./dist/index.js",
              "default": "./dist/index.js"
            }
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/#alias"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/#alias uses an invalid key. exports keys must not start with #.",
        "Rename the exports entry to . or ./subpath, or use a condition key without a # prefix.",
        Some(fixture.path().join("package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/./#nested"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/./#nested uses an invalid key. exports keys must not start with #.",
        "Rename the exports entry to . or ./subpath, or use a condition key without a # prefix.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_mixed_root_exports_subpath_and_condition_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            ".": "./dist/index.js",
            "import": "./dist/index.mjs"
          }
        }
        "##,
    );
    write(fixture.path().join("dist/index.js"), "module.exports = {};\n");
    write(fixture.path().join("dist/index.mjs"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "package-entrypoints:{}:exports:mixed-keys",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Error,
        "Package exports object is invalid",
        "package.json exports mixes subpath keys with condition keys at the same object level.",
        "Use either package subpath keys such as . and ./feature, or conditional keys such as import and default at one level.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_exports_subpath_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./../bad": "./dist/bad.js",
            "./ok/path": "./dist/ok.js"
          }
        }
        "##,
    );
    write(fixture.path().join("dist/bad.js"), "module.exports = {};\n");
    write(fixture.path().join("dist/ok.js"), "module.exports = {};\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/./../bad"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/./../bad uses an invalid key. exports subpath keys must not contain empty, ., .., or node_modules path segments.",
        "Rename the exports entry to . or a ./subpath that stays within the package.",
        Some(fixture.path().join("package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/./ok/path"),
    );
}

#[test]
fn package_entrypoints_check_reports_trailing_slash_exports_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./feature/": "./feature/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("feature/index.js"),
        "module.exports = {};\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "exports/./feature/"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json exports/./feature/ uses an invalid key. exports subpath keys must not contain empty, ., .., or node_modules path segments.",
        "Rename the exports entry to . or a ./subpath that stays within the package.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_empty_named_import_aliases() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#": "./src/index.js"
          }
        }
        "##,
    );
    write(fixture.path().join("src/index.js"), "export default 1;\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id(fixture.path(), "imports/#"),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/# uses an invalid key. imports keys must include a name after the # prefix.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_nested_import_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#nested": "./dist/module"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "imports": {
            "react": "./src/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/src/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "imports/react",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("dist/module/package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "imports/#nested", "./dist/module"),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_nested_import_keys_for_main_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./entry.js",
          "imports": {
            "react": "./src/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/entry.js"),
        "module.exports = {}\n",
    );
    write(
        fixture.path().join("dist/module/src/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "imports/react",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("dist/module/package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
    );
}

#[test]
fn package_entrypoints_check_preserves_findings_from_deeper_nested_package_manifests() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./subdir"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/subdir/package.json"),
        r##"
        {
          "main": "./index.js",
          "imports": {
            "react": "./shim.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/subdir/index.js"),
        "module.exports = {}\n",
    );
    write(
        fixture.path().join("dist/module/subdir/shim.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/module/subdir/package.json"),
            "imports/react",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("dist/module/subdir/package.json")),
    );
    assert_no_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
    );
}

#[test]
fn package_entrypoints_check_reports_incompatible_nested_exact_file_types() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./index.d.ts"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/index.d.ts"),
        "export type Maximus = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "main",
            "./index.d.ts",
        ),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json main points to ./index.d.ts, but runtime entrypoints must not point to declaration-only files such as .d.ts.",
        "Point main/module/bin to a runtime file instead of a declaration-only file.",
        Some(fixture.path().join("dist/module/package.json")),
    );
}

#[test]
fn package_entrypoints_check_preserves_nested_import_key_findings_when_directory_target_still_fails(
) {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./missing.js",
          "imports": {
            "react": "./src/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/src/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/module/package.json"),
            "imports/react",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("dist/module/package.json")),
    );
    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/module, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_invalid_nested_import_keys_for_wildcard_exports() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./*": "./dist/*"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/pkg/package.json"),
        r##"
        {
          "imports": {
            "react": "./src/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/pkg/src/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &key_finding_id_for_package(
            &fixture.path().join("dist/pkg/package.json"),
            "imports/react",
        ),
        Severity::Error,
        "Package entrypoint key is invalid",
        "package.json imports/react uses an invalid key. imports keys must start with #.",
        "Rename the imports entry to a # alias before publishing.",
        Some(fixture.path().join("dist/pkg/package.json")),
    );
}

#[test]
fn package_entrypoints_check_rejects_wildcard_matches_that_only_hit_package_manifests() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./*": "./dist/*"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/pkg/package.json"),
        r##"
        {
          "name": "pkg",
          "version": "0.0.0"
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./*", "./dist/*"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json exports/./* points to ./dist/*, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_skips_current_manifest_when_wildcard_matches_package_json() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./*": "./*"
          }
        }
        "##,
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "exports/./*", "./*"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json exports/./* points to ./*, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_reports_incompatible_wildcard_runtime_matches() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "exports": {
            "./*": "./dist/*"
          }
        }
        "##,
    );
    write(
        fixture.path().join("dist/index.d.ts"),
        "export type Maximus = string;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "package-entrypoints:{}:exports/./*:./dist/*:./dist/index.d.ts",
            fixture.path().join("package.json").to_string_lossy()
        ),
        Severity::Error,
        "Package entrypoint target uses an incompatible file type",
        "package.json exports/./* points to ./dist/*, but wildcard match ./dist/index.d.ts is a declaration-only file.",
        "Point runtime entrypoints to runtime files instead of declaration-only files.",
        Some(fixture.path().join("package.json")),
    );
}

#[test]
fn package_entrypoints_check_deduplicates_nested_import_key_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "imports": {
            "#nested": "./packages/foo"
          }
        }
        "##,
    );
    write(
        fixture.path().join("packages/foo/package.json"),
        r##"
        {
          "name": "foo",
          "version": "0.0.0",
          "imports": {
            "react": "./src/index.js"
          }
        }
        "##,
    );
    write(
        fixture.path().join("packages/foo/src/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");
    let duplicate_id = key_finding_id_for_package(
        &fixture.path().join("packages/foo/package.json"),
        "imports/react",
    );
    let duplicate_count = outcome
        .findings
        .iter()
        .filter(|finding| finding.id == duplicate_id)
        .count();

    assert_eq!(duplicate_count, 1);
}

#[test]
fn package_entrypoints_check_treats_invalid_nested_package_json_as_missing_target() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r##"
        {
          "main": "./dist/module"
        }
        "##,
    );
    write(
        fixture.path().join("dist/module/package.json"),
        r##"
        {
          "main": "./entry.js"
        "##,
    );
    write(
        fixture.path().join("dist/module/index.js"),
        "export default 1;\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_package_entrypoints_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &finding_id(fixture.path(), "main", "./dist/module"),
        Severity::Error,
        "Package entrypoint target does not exist",
        "package.json main points to ./dist/module, but the resolved path was not found.",
        "Update the relative path or remove the stale entrypoint before publishing.",
        Some(fixture.path().join("package.json")),
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn finding_id(root: &Path, field_path: &str, target: &str) -> String {
    finding_id_for_package(&root.join("package.json"), field_path, target)
}

fn finding_id_for_package(package_json_path: &Path, field_path: &str, target: &str) -> String {
    format!(
        "package-entrypoints:{}:{}:{}",
        package_json_path.to_string_lossy(),
        field_path,
        target
    )
}

fn key_finding_id(root: &Path, field_path: &str) -> String {
    key_finding_id_for_package(&root.join("package.json"), field_path)
}

fn key_finding_id_for_package(package_json_path: &Path, field_path: &str) -> String {
    format!(
        "package-entrypoints:{}:{}:key",
        package_json_path.to_string_lossy(),
        field_path
    )
}

fn invalid_type_finding_id(root: &Path, field_path: &str, value_kind: &str) -> String {
    invalid_type_finding_id_for_package(&root.join("package.json"), field_path, value_kind)
}

fn invalid_type_finding_id_for_package(
    package_json_path: &Path,
    field_path: &str,
    value_kind: &str,
) -> String {
    format!(
        "package-entrypoints:{}:{}:invalid-type:{}",
        package_json_path.to_string_lossy(),
        field_path,
        value_kind
    )
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
        .unwrap_or_else(|| {
            let ids = findings
                .iter()
                .map(|finding| finding.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            panic!("missing finding {id}; available ids: {ids}");
        });

    assert_eq!(finding.severity, severity);
    assert_eq!(finding.title, title);
    assert_eq!(finding.detail, detail);
    assert_eq!(finding.hint, hint);
    assert_eq!(finding.file, file);
    assert!(!finding.fixable);
    assert!(finding.fix_ids.is_empty());
}

fn assert_no_finding(findings: &[maximus_core::Finding], id: &str) {
    assert!(
        findings.iter().all(|finding| finding.id != id),
        "unexpected finding {id}"
    );
}
