use std::fs;
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
    assert_eq!(
        finding_field(
            findings[0]
                .as_object()
                .expect("finding should be an object"),
            "severity"
        ),
        "warn"
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
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("Maximus failed:"));
    assert!(stderr.contains(&config_path.to_string_lossy().to_string()));
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
        assert!(stderr.contains(&format!("Option \"{flag}\" requires a non-empty value.")));
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
            "Unknown check id \"not-a-real-check\" in only.",
        ),
        (
            r#"{ "checks": { "skip": ["not-a-real-check"] } }"#,
            vec!["--only", "env"],
            "Unknown check id \"not-a-real-check\" in skip.",
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

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
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
