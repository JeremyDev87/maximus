use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn maximus_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

#[test]
fn root_maximus_config_applies_check_defaults() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["schemaVersion"], "1");
    assert_eq!(value["generator"], "maximus");
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(finding_field(
        findings[0]
            .as_object()
            .expect("finding should be an object"),
        "id"
    )
    .starts_with("env-mismatch:"));
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "env"
    );
}

#[cfg(unix)]
#[test]
fn config_applies_through_symlinked_target_path() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let real_root = fixture.path().join("real");
    let real_target = real_root.join("apps/web");
    let alias_target = fixture.path().join("alias-web");
    write_mixed_fixture(&real_target);
    write(
        real_root.join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );
    std::os::unix::fs::symlink(&real_target, &alias_target).expect("symlink should create");

    let output = maximus_bin()
        .args(["audit", alias_target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "env"
    );
}

#[cfg(unix)]
#[test]
fn root_relative_ignore_applies_through_symlinked_nested_target() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let real_root = fixture.path().join("real");
    let real_target = real_root.join("packages/web");
    let alias_target = fixture.path().join("web-alias");
    write_tsconfig_conflict_fixture(&real_target.join("generated"));
    write(
        real_root.join("maximus.config.json"),
        r#"{ "ignore": ["packages/web/generated"], "checks": { "only": ["tsconfig"] } }"#,
    );
    std::os::unix::fs::symlink(&real_target, &alias_target).expect("symlink should create");

    let output = maximus_bin()
        .args(["audit", alias_target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "root-relative ignore should suppress nested symlink findings: {findings:?}"
    );
}

#[cfg(unix)]
#[test]
fn root_relative_lockfiles_ignore_applies_through_symlinked_nested_target() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let real_root = fixture.path().join("real");
    let real_target = real_root.join("packages/web");
    let alias_target = fixture.path().join("web-alias");
    write(real_target.join("ignored/package-lock.json"), "{}\n");
    write(
        real_target.join("ignored/yarn.lock"),
        "# yarn lockfile v1\n",
    );
    write(
        real_root.join("maximus.config.json"),
        r#"{ "ignore": ["packages/web/ignored"], "checks": { "only": ["lockfiles"] } }"#,
    );
    std::os::unix::fs::symlink(&real_target, &alias_target).expect("symlink should create");

    let output = maximus_bin()
        .args(["audit", alias_target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "root-relative ignore should suppress lockfile findings via symlink target: {findings:?}"
    );
}

#[test]
fn direct_ignored_target_path_produces_no_tsconfig_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target = fixture.path().join("packages/web/generated");
    write_tsconfig_conflict_fixture(&target);
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignore": ["packages/web/generated"], "checks": { "only": ["tsconfig"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignored direct target should suppress tsconfig findings: {findings:?}"
    );
}

#[test]
fn direct_ignored_target_path_produces_no_lockfile_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target = fixture.path().join("packages/web/generated");
    write(target.join("package-lock.json"), "{}\n");
    write(target.join("yarn.lock"), "# yarn lockfile v1\n");
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignore": ["packages/web/generated"], "checks": { "only": ["lockfiles"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignored direct target should suppress lockfile findings: {findings:?}"
    );
}

#[cfg(unix)]
#[test]
fn lexical_mount_config_does_not_override_realpath_config() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let real_root = fixture.path().join("real");
    let real_target = real_root.join("apps/web");
    let mount_root = fixture.path().join("mount");
    let alias_target = mount_root.join("web");
    write_mixed_fixture(&real_target);
    write(
        real_root.join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );
    write(
        mount_root.join("maximus.config.json"),
        r#"{ "checks": { "only": ["tsconfig"] } }"#,
    );
    fs::create_dir_all(&mount_root).expect("mount dir should exist");
    std::os::unix::fs::symlink(&real_target, &alias_target).expect("symlink should create");

    let output = maximus_bin()
        .args(["audit", alias_target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "env"
    );
}

#[test]
fn nested_maximusrc_takes_precedence_for_nested_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let nested = fixture.path().join("apps/web");
    write_mixed_fixture(&nested);
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );
    write(
        nested.join(".maximusrc.json"),
        r#"{ "checks": { "only": ["tsconfig"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", nested.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "tsconfig"
    );
    assert!(finding_field(
        findings[0]
            .as_object()
            .expect("finding should be an object"),
        "id"
    )
    .starts_with("tsconfig-import-conflict:"));
}

#[test]
fn cli_only_flag_overrides_config_defaults() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "tsconfig"
    );
}

#[test]
fn cli_only_flag_clears_config_skip_filters() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "checks": { "skip": ["env"] } }"#,
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "category"
        ),
        "env"
    );
}

#[test]
fn cli_skip_flag_clears_config_only_filters() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "checks": { "only": ["env"] } }"#,
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--skip",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(
        findings.iter().any(|finding| {
            finding_field(
                finding.as_object().expect("finding should be an object"),
                "category",
            ) == "tsconfig"
        }),
        "expected at least one tsconfig finding: {findings:?}"
    );
}

#[test]
fn config_glob_ignore_and_severity_overrides_are_applied() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_env_fixture(fixture.path());
    write_tsconfig_conflict_fixture(&fixture.path().join("packages/web/generated"));
    write(
        fixture.path().join("maximus.config.json"),
        r#"
        {
          "ignore": ["**/generated"],
          "severity": {
            "env-mismatch": "error"
          }
        }
        "#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(finding_field(
        findings[0]
            .as_object()
            .expect("finding should be an object"),
        "id"
    )
    .starts_with("env-mismatch:"));
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "severity"
        ),
        "error"
    );
}

#[test]
fn config_suppression_by_exact_finding_id_hides_finding_and_counts_summary() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    let finding_id = tsconfig_import_conflict_id(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id,
                    "reason": "fixture intentionally keeps runtime/editor aliases split"
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["totalFindings"], 0);
    assert_eq!(value["summary"]["warningFindings"], 0);
    assert_eq!(value["summary"]["suppressedByConfig"], 1);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(findings.is_empty(), "suppressed finding should be hidden");
}

#[test]
fn config_suppression_file_prefix_restricts_matching_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target = fixture.path().join("packages/web");
    write_tsconfig_conflict_fixture(&target);
    let finding_id = tsconfig_import_conflict_id(&target);
    write(
        fixture.path().join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id,
                    "filePrefix": "packages/web"
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["suppressedByConfig"], 1);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "matching filePrefix should hide finding: {findings:?}"
    );
}

#[test]
fn root_config_suppression_file_prefix_matches_nested_audit_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let target = repo.join("packages/web");
    write_tsconfig_conflict_fixture(&target);
    let finding_id = tsconfig_import_conflict_id(&target);
    write(
        repo.join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id,
                    "filePrefix": "packages/web"
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["suppressedByConfig"], 1);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "root config filePrefix should match nested target finding: {findings:?}"
    );
}

#[test]
fn nested_config_suppression_file_prefix_matches_deeper_audit_targets() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let config_root = repo.join("packages/web");
    let target = config_root.join("app");
    fs::create_dir_all(repo.join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&target);
    let finding_id = tsconfig_import_conflict_id(&target);
    write(
        config_root.join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id,
                    "filePrefix": "app"
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["suppressedByConfig"], 1);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "nested config filePrefix should match deeper target finding: {findings:?}"
    );
}

#[test]
fn config_suppression_with_non_matching_file_prefix_keeps_finding() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target = fixture.path().join("packages/web");
    write_tsconfig_conflict_fixture(&target);
    let finding_id = tsconfig_import_conflict_id(&target);
    write(
        fixture.path().join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id,
                    "filePrefix": "packages/api"
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["suppressedByConfig"], 0);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "id"
        ),
        finding_id
    );
}

#[test]
fn config_suppression_removes_fixes_for_hidden_findings() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(fixture.path().join(".env"), "API_URL=http://localhost\n");
    write(fixture.path().join(".gitignore"), ".env\n");
    let finding_id = env_example_missing_id(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["env"] },
            "suppressions": [
                {
                    "id": finding_id
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    assert_eq!(value["summary"]["fixesAvailable"], 0);
    assert_eq!(value["summary"]["suppressedByConfig"], 1);
    let fixes = value["fixes"].as_array().expect("fixes should be an array");
    assert!(
        fixes.is_empty(),
        "suppressed finding fixes should be hidden"
    );
}

#[test]
fn config_suppression_text_report_shows_nonzero_suppressed_count() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    let finding_id = tsconfig_import_conflict_id(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        &serde_json::to_string_pretty(&serde_json::json!({
            "checks": { "only": ["tsconfig"] },
            "suppressions": [
                {
                    "id": finding_id
                }
            ]
        }))
        .expect("config json should render"),
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref()])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("설정으로 숨김: 1개"),
        "text report should show nonzero suppressed count: {stdout}"
    );
}

#[test]
fn config_ignore_patterns_alias_applies_to_discovery() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(&fixture.path().join("generated"));
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignorePatterns": ["generated"], "checks": { "only": ["tsconfig"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignorePatterns should suppress generated findings: {findings:?}"
    );
}

#[test]
fn maximusignore_applies_to_discovery_without_config_file() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(&fixture.path().join("generated"));
    write(fixture.path().join(".maximusignore"), "generated/\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        ".maximusignore should suppress generated findings: {findings:?}"
    );
}

#[test]
fn ancestor_gitignore_applies_when_auditing_nested_target() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let target = repo.join("packages/web");
    fs::create_dir_all(repo.join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&target.join("generated"));
    write(repo.join(".gitignore"), "packages/web/generated/\n");

    let output = maximus_bin()
        .args([
            "audit",
            target.to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ancestor .gitignore should suppress generated findings: {findings:?}"
    );
}

#[test]
fn gitignore_root_and_nested_config_relative_patterns_are_combined() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let target = repo.join("packages/web");
    fs::create_dir_all(repo.join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&target.join("generated"));
    write_tsconfig_conflict_fixture(&target.join("local/generated"));
    write(repo.join(".gitignore"), "packages/web/generated/\n");
    write(
        target.join("maximus.config.json"),
        r#"{ "ignorePatterns": ["local/generated"], "checks": { "only": ["tsconfig"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "root .gitignore and nested config ignorePatterns should both apply: {findings:?}"
    );
}

#[test]
fn nested_gitignore_bare_pattern_applies_when_auditing_inside_ignored_directory() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let target = repo.join("packages/web/generated/sub");
    fs::create_dir_all(repo.join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&target);
    write(repo.join("packages/web/.gitignore"), "generated\n");

    let output = maximus_bin()
        .args([
            "audit",
            target.to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "nested .gitignore bare pattern should suppress direct child audit target: {findings:?}"
    );
}

#[test]
fn nested_config_bare_ignore_applies_when_auditing_inside_ignored_directory() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let config_root = repo.join("packages/web");
    let target = config_root.join("generated/sub");
    write_tsconfig_conflict_fixture(&target);
    write(
        config_root.join("maximus.config.json"),
        r#"{ "ignore": ["generated"], "checks": { "only": ["tsconfig"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", target.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "nested config bare ignore should suppress direct child audit target: {findings:?}"
    );
}

#[test]
fn gitignore_patterns_apply_to_lockfiles_check_traversal() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(fixture.path().join("ignored/package-lock.json"), "{}\n");
    write(
        fixture.path().join("ignored/yarn.lock"),
        "# yarn lockfile v1\n",
    );
    write(fixture.path().join(".gitignore"), "ignored/\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "lockfiles",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        ".gitignore should suppress ignored lockfile findings: {findings:?}"
    );
}

#[test]
fn gitignore_patterns_do_not_hide_env_check_inputs() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(fixture.path().join(".env.local"), "SECRET=local\n");
    write(fixture.path().join(".gitignore"), ".env.local\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().any(|finding| {
            finding["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("env-example-missing:"))
        }),
        ".gitignore should not remove env files from env contract checks: {findings:?}"
    );
}

#[test]
fn gitignore_patterns_do_not_hide_tracked_env_check_inputs() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(fixture.path().join(".env"), "SECRET=tracked\n");
    write(fixture.path().join(".gitignore"), ".env\n");
    run_git(fixture.path(), &["init"]);
    run_git(fixture.path(), &["add", "-f", ".env"]);

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().any(|finding| {
            finding["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("env-gitignore:"))
        }),
        "tracked env files ignored by .gitignore should still reach the tracked-file guard: {findings:?}"
    );
}

#[test]
fn gitignore_suffix_env_globs_do_not_hide_env_check_inputs() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(fixture.path().join(".env"), "SECRET=one\nSECRET=two\n");
    write(
        fixture.path().join("archive.env/.env"),
        "ARCHIVE_SECRET=one\nARCHIVE_SECRET=two\n",
    );
    write(fixture.path().join(".gitignore"), "*.env\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let root_env = fixture.path().join(".env").to_string_lossy().into_owned();
    let archive_env = fixture
        .path()
        .join("archive.env/.env")
        .to_string_lossy()
        .into_owned();
    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().any(|finding| {
            finding["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&format!("env-duplicate:{root_env}:SECRET:")))
        }),
        "suffix-style gitignore globs should not remove env files from env checks: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| !finding.to_string().contains(&archive_env)),
        "suffix-style gitignore globs should still hide matching directories: {findings:?}"
    );
}

#[test]
fn gitignore_env_globs_with_variable_suffixes_do_not_hide_env_check_inputs() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(
        fixture.path().join(".env.production"),
        "PRODUCTION_SECRET=one\nPRODUCTION_SECRET=two\n",
    );
    write(
        fixture.path().join(".env.production.local"),
        "LOCAL_SECRET=one\nLOCAL_SECRET=two\n",
    );
    write(
        fixture.path().join(".gitignore"),
        ".env.prod*\n.env.*.local\n",
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let production_env = fixture
        .path()
        .join(".env.production")
        .to_string_lossy()
        .into_owned();
    let local_env = fixture
        .path()
        .join(".env.production.local")
        .to_string_lossy()
        .into_owned();
    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().any(|finding| {
            finding["id"].as_str().is_some_and(|id| {
                id.starts_with(&format!(
                    "env-duplicate:{production_env}:PRODUCTION_SECRET:"
                ))
            })
        }),
        "env glob patterns should not remove .env.production from env checks: {findings:?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["id"].as_str().is_some_and(|id| {
                id.starts_with(&format!("env-duplicate:{local_env}:LOCAL_SECRET:"))
            })
        }),
        "env glob patterns should not remove .env.production.local from env checks: {findings:?}"
    );
}

#[test]
fn broad_env_globs_still_hide_matching_directories() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(
        fixture.path().join("old-env-copy/.env"),
        "LANE_SECRET=one\nLANE_SECRET=two\n",
    );
    write(fixture.path().join(".gitignore"), "*env*\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let ignored_env = fixture
        .path()
        .join("old-env-copy/.env")
        .to_string_lossy()
        .into_owned();
    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings
            .iter()
            .all(|finding| !finding.to_string().contains(&ignored_env)),
        "broad env globs should still hide matching directories: {findings:?}"
    );
}

#[test]
fn gitignore_negated_env_patterns_are_kept_for_env_check_rediscovery() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(fixture.path().join(".env"), "SECRET=one\n");
    write(fixture.path().join(".env.example"), "SECRET=\n");
    write(
        fixture.path().join(".gitignore"),
        ".env\n*.example\n!.env.example\n",
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().all(|finding| {
            finding["id"].as_str().is_none_or(|id| {
                !id.starts_with(&format!(
                    "env-example-missing:{}",
                    fixture.path().to_string_lossy()
                ))
            })
        }),
        "negated env patterns should keep whitelisted env templates visible: {findings:?}"
    );
}

#[test]
fn gitignore_env_patterns_do_not_reinclude_worktree_env_files() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write(fixture.path().join(".env"), "SECRET=one\nSECRET=two\n");
    write(fixture.path().join(".env.local.example"), "SECRET=\n");
    write(
        fixture.path().join(".worktrees/lane/.env"),
        "LANE_SECRET=one\nLANE_SECRET=two\n",
    );
    write(
        fixture.path().join(".gitignore"),
        ".env*\n!.env.local.example\n.worktrees/\n",
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "env",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let root_env = fixture.path().join(".env").to_string_lossy().into_owned();
    let worktree_env = fixture
        .path()
        .join(".worktrees/lane/.env")
        .to_string_lossy()
        .into_owned();
    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.iter().any(|finding| {
            finding["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&format!("env-duplicate:{root_env}:SECRET:")))
        }),
        "root .env duplicate should remain visible to env checks: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .all(|finding| !finding.to_string().contains(&worktree_env)),
        ".worktrees env copies should stay excluded from env checks: {findings:?}"
    );
}

#[test]
fn config_ignore_patterns_hide_vitest_config_from_test_runner_check() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("package.json"),
        r#"{ "name": "fixture" }"#,
    );
    write(
        fixture.path().join("jest.config.js"),
        "module.exports = {};\n",
    );
    write(
        fixture.path().join("vitest.config.ts"),
        "export default {};\n",
    );
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignorePatterns": ["vitest.config.ts"], "checks": { "only": ["test-runner-config"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignorePatterns should suppress ignored vitest config findings: {findings:?}"
    );
}

#[test]
fn config_ignore_patterns_hide_root_editorconfig_from_editorconfig_prettier_check() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join(".editorconfig"),
        "root = true\nindent_style = tab\nindent_size = 4\nend_of_line = crlf\n",
    );
    write(
        fixture.path().join(".prettierrc"),
        r#"{ "useTabs": false, "tabWidth": 2, "endOfLine": "lf" }"#,
    );
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignorePatterns": [".editorconfig"], "checks": { "only": ["editorconfig-prettier"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignorePatterns should suppress ignored EditorConfig findings: {findings:?}"
    );
}

#[test]
fn gitignore_escaped_leading_bang_matches_literal_path() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&fixture.path().join("!generated"));
    write_tsconfig_conflict_fixture(&fixture.path().join("generated"));
    write(fixture.path().join(".gitignore"), "\\!generated/\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    let finding = findings[0]
        .as_object()
        .expect("finding should be an object");
    assert!(
        finding_field(finding, "file").contains("generated/tsconfig.json"),
        "escaped bang should suppress literal !generated only: {findings:?}"
    );
    assert!(
        !finding_field(finding, "file").contains("!generated/tsconfig.json"),
        "escaped bang should not scan literal !generated: {findings:?}"
    );
}

#[test]
fn gitignore_anchored_root_pattern_does_not_suppress_nested_target() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let repo = fixture.path().join("repo");
    let target = repo.join("packages/web");
    fs::create_dir_all(repo.join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(&target.join("generated"));
    write(repo.join(".gitignore"), "/generated/\n");

    let output = maximus_bin()
        .args([
            "audit",
            target.to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "id"
        )
        .starts_with("tsconfig-import-conflict:"),
        "anchored root .gitignore should not suppress nested generated findings: {findings:?}"
    );
}

#[test]
fn gitignore_directory_only_file_pattern_does_not_suppress_matching_file() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    write(fixture.path().join(".gitignore"), "tsconfig.json/\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "id"
        )
        .starts_with("tsconfig-import-conflict:"),
        "directory-only .gitignore file pattern should not suppress a file: {findings:?}"
    );
}

#[test]
fn gitignore_leading_space_pattern_does_not_suppress_matching_file() {
    let fixture = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(fixture.path().join(".git")).expect("git dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    write(fixture.path().join(".gitignore"), " tsconfig.json\n");

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--only",
            "tsconfig",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "id"
        )
        .starts_with("tsconfig-import-conflict:"),
        "leading-space .gitignore pattern should not suppress tsconfig.json: {findings:?}"
    );
}

#[test]
fn config_ignore_applies_to_lockfiles_check_traversal() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(fixture.path().join("ignored/package-lock.json"), "{}\n");
    write(
        fixture.path().join("ignored/yarn.lock"),
        "# yarn lockfile v1\n",
    );
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "ignore": ["ignored"], "checks": { "only": ["lockfiles"] } }"#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert!(
        findings.is_empty(),
        "ignored lockfiles should not produce findings: {findings:?}"
    );
}

#[test]
fn config_fail_on_can_be_overridden_by_cli() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_env_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"{ "report": { "failOn": "info" } }"#,
    );

    let config_output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");
    assert_eq!(config_output.status.code(), Some(1), "{config_output:?}");

    let cli_override_output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--fail-on",
            "none",
            "--json",
        ])
        .output()
        .expect("audit override should run");
    assert_eq!(
        cli_override_output.status.code(),
        Some(0),
        "{cli_override_output:?}"
    );
}

#[test]
fn empty_severity_prefix_is_ignored() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_tsconfig_conflict_fixture(fixture.path());
    write(
        fixture.path().join("maximus.config.json"),
        r#"
        {
          "severity": {
            "": "error"
          },
          "report": {
            "failOn": "error"
          }
        }
        "#,
    );

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(
        findings.iter().any(|finding| {
            finding_field(
                finding.as_object().expect("finding should be an object"),
                "severity",
            ) == "warn"
        }),
        "expected original warn finding to remain: {findings:?}"
    );
}

#[test]
fn broken_config_is_reported_as_cli_error() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let config_path = fixture.path().join("maximus.config.json");
    write(&config_path, r#"{ "checks": { "only": ["env",] }"#);

    let output = maximus_bin()
        .args(["audit", fixture.path().to_string_lossy().as_ref()])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr should be utf8");
    assert!(stderr.contains("Maximus 실패:"));
    assert!(stderr.contains(&config_path.to_string_lossy().to_string()));
}

#[test]
fn composite_project_reference_is_blocking_under_fail_on_error() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join("tsconfig.json"),
        r#"
        {
          "references": [
            { "path": "./packages/pkg-a" }
          ]
        }
        "#,
    );
    write(
        fixture.path().join("packages/pkg-a/tsconfig.json"),
        r#"{ "compilerOptions": { "declaration": true } }"#,
    );

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().to_string_lossy().as_ref(),
            "--fail-on",
            "error",
            "--json",
        ])
        .output()
        .expect("audit should run");

    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "severity"
        ),
        "error"
    );
    assert!(finding_field(
        findings[0]
            .as_object()
            .expect("finding should be an object"),
        "id"
    )
    .contains(":composite"));
}

#[cfg(unix)]
#[test]
fn unreadable_project_reference_becomes_a_finding_instead_of_a_cli_error() {
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

    let output = maximus_bin()
        .args([
            "audit",
            fixture.path().join("root").to_string_lossy().as_ref(),
            "--json",
        ])
        .output()
        .expect("audit should run");

    let mut restore_permissions = original_permissions;
    restore_permissions.set_mode(0o644);
    fs::set_permissions(&target_path, restore_permissions)
        .expect("target permissions should restore");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr should be utf8");
    assert!(
        stderr.is_empty(),
        "permission failures should be reported as findings, not fatal CLI errors: {stderr}"
    );

    let value = parse_json(&output);
    let findings = value["findings"]
        .as_array()
        .expect("findings should be an array");
    assert_eq!(findings.len(), 1);
    assert!(finding_field(
        findings[0]
            .as_object()
            .expect("finding should be an object"),
        "id"
    )
    .contains(":unreadable"));
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "severity"
        ),
        "error"
    );
}

#[test]
fn empty_cli_check_filters_are_reported_as_cli_errors() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());

    for flag in ["--only", "--skip"] {
        let output = maximus_bin()
            .args([
                "audit",
                fixture.path().to_string_lossy().as_ref(),
                flag,
                " , ",
            ])
            .output()
            .expect("audit should run");

        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(&format!(
            "\"{flag}\" 옵션에는 비어 있지 않은 값이 필요합니다."
        )));
    }
}

#[test]
fn cli_filters_do_not_hide_invalid_config_check_ids() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write_mixed_fixture(fixture.path());

    for (config_body, cli_args, expected_fragment) in [
            (
                r#"{ "checks": { "only": ["not-a-real-check"] } }"#,
                vec!["--skip", "env"],
                "only에 알 수 없는 check id \"not-a-real-check\"가 있습니다.",
            ),
            (
                r#"{ "checks": { "skip": ["not-a-real-check"] } }"#,
                vec!["--only", "env"],
                "skip에 알 수 없는 check id \"not-a-real-check\"가 있습니다.",
            ),
    ] {
        write(fixture.path().join("maximus.config.json"), config_body);

        let output = maximus_bin()
            .args(["audit", fixture.path().to_string_lossy().as_ref()])
            .args(cli_args)
            .output()
            .expect("audit should run");

        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
        assert!(stderr.contains(expected_fragment), "{stderr}");
    }
}

fn write_mixed_fixture(root: &Path) {
    write_env_fixture(root);
    write_tsconfig_conflict_fixture(root);
}

fn write_env_fixture(root: &Path) {
    write(root.join(".env"), "SHARED=base\n");
    write(root.join(".env.local"), "SHARED=local\n");
    write(root.join(".env.example"), "SHARED=\n");
    write(root.join(".gitignore"), ".env\n.env.local\n");
}

fn write_tsconfig_conflict_fixture(root: &Path) {
    write(
        root.join("package.json"),
        r##"{"name":"fixture","imports":{"#app/*":"./src/runtime/*"}}"##,
    );
    write(
        root.join("tsconfig.json"),
        r##"
        {
          "compilerOptions": {
            "baseUrl": ".",
            "paths": {
              "#app/*": ["./src/lib/*"]
            }
          }
        }
        "##,
    );
    write(root.join("src/runtime/index.ts"), "export {};\n");
    write(root.join("src/lib/index.ts"), "export {};\n");
}

fn tsconfig_import_conflict_id(root: &Path) -> String {
    format!(
        "tsconfig-import-conflict:{}:#app/*",
        root.join("tsconfig.json").to_string_lossy()
    )
}

fn env_example_missing_id(root: &Path) -> String {
    format!("env-example-missing:{}", root.to_string_lossy())
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git should run");
    assert!(output.status.success(), "{output:?}");
}

fn parse_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

fn finding_field<'a>(finding: &'a serde_json::Map<String, Value>, key: &str) -> &'a str {
    finding
        .get(key)
        .and_then(Value::as_str)
        .expect("finding field should be a string")
}
