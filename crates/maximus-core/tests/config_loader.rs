use std::fs;

use maximus_core::{
    find_maximus_config_path, load_maximus_config, ConfigSeverity, FailOnLevel,
};
use tempfile::tempdir;

#[test]
fn finds_root_maximus_config_when_searching_nested_directory() {
    let temp = tempdir().expect("temp dir should exist");
    let nested = temp.path().join("apps/web");
    fs::create_dir_all(&nested).expect("nested dir should exist");
    fs::write(temp.path().join("maximus.config.json"), "{ \"ignore\": [\"dist\"] }")
        .expect("config should write");

    let found = find_maximus_config_path(&nested).expect("search should succeed");

    assert_eq!(found, Some(temp.path().join("maximus.config.json")));
}

#[test]
fn prefers_nearest_config_and_then_file_name_precedence() {
    let temp = tempdir().expect("temp dir should exist");
    let nested = temp.path().join("apps/web");
    fs::create_dir_all(&nested).expect("nested dir should exist");
    fs::write(temp.path().join("maximus.config.json"), "{ \"ignore\": [\"root\"] }")
        .expect("root config should write");
    fs::write(nested.join(".maximusrc.json"), "{ \"ignore\": [\"nested\"] }")
        .expect("nested config should write");
    fs::write(
        nested.join("maximus.config.json"),
        "{ \"ignore\": [\"nested-maximus\"] }",
    )
    .expect("nested maximus config should write");

    let loaded = load_maximus_config(&nested)
        .expect("load should succeed")
        .expect("config should exist");

    assert_eq!(loaded.path, nested.join("maximus.config.json"));
    assert_eq!(loaded.config.ignore, vec!["nested-maximus".to_string()]);
}

#[test]
fn parses_jsonc_shape_for_checks_severity_and_report() {
    let temp = tempdir().expect("temp dir should exist");
    fs::write(
        temp.path().join("maximus.config.json"),
        r#"
        {
          // comment
          "checks": {
            "only": ["env", "tsconfig"],
            "skip": ["duplicates"]
          },
          "ignore": ["dist", "coverage"],
          "severity": {
            "env-mismatch": "info"
          },
          "report": {
            "failOn": "error"
          }
        }
        "#,
    )
    .expect("config should write");

    let loaded = load_maximus_config(temp.path())
        .expect("load should succeed")
        .expect("config should exist");

    assert_eq!(loaded.config.checks.only, vec!["env", "tsconfig"]);
    assert_eq!(loaded.config.checks.skip, vec!["duplicates"]);
    assert_eq!(loaded.config.ignore, vec!["dist", "coverage"]);
    assert_eq!(
        loaded.config.severity.get("env-mismatch"),
        Some(&ConfigSeverity::Info)
    );
    assert_eq!(loaded.config.report.fail_on, Some(FailOnLevel::Error));
}

#[test]
fn returns_none_when_no_config_exists() {
    let temp = tempdir().expect("temp dir should exist");

    assert_eq!(find_maximus_config_path(temp.path()).expect("search should succeed"), None);
    assert!(load_maximus_config(temp.path()).expect("load should succeed").is_none());
}

#[test]
fn parse_errors_include_config_path_label() {
    let temp = tempdir().expect("temp dir should exist");
    let config_path = temp.path().join(".maximusrc.json");
    fs::write(&config_path, "{ \"checks\": { \"only\": [\"env\",] }")
        .expect("broken config should write");

    let error = load_maximus_config(temp.path()).expect_err("parse should fail");
    let rendered = error.to_string();

    assert!(rendered.contains(&config_path.to_string_lossy().to_string()));
}
