#![allow(dead_code)]

#[path = "../src/text_order.rs"]
mod text_order;
#[path = "../src/env_parser.rs"]
mod env_parser;
#[path = "../src/fs.rs"]
mod fs;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/fixes.rs"]
mod fixes;

use std::fs as stdfs;

use tempfile::tempdir;

use fixes::{apply_fix, apply_fixes, plan_create_env_example, plan_sync_env_example, FixOperation};

#[test]
fn create_env_example_plan_matches_js_metadata_contract() {
    let root = tempdir().expect("temp dir should exist");
    let fix = plan_create_env_example(
        root.path(),
        root.path(),
        &["AUTH_TOKEN".to_string(), "API_URL".to_string()],
    );

    assert_eq!(fix.public.id, format!("env-example:create:{}", root.path().to_string_lossy()));
    assert_eq!(fix.public.title, "Create .env.example");
    assert_eq!(fix.public.files, vec![root.path().join(".env.example")]);

    match &fix.operation {
        FixOperation::CreateEnvExample { output_path, keys } => {
            assert_eq!(output_path, &root.path().join(".env.example"));
            assert_eq!(keys, &vec!["AUTH_TOKEN".to_string(), "API_URL".to_string()]);
        }
        _ => panic!("expected create env example operation"),
    }
}

#[test]
fn apply_create_env_example_writes_sorted_template() {
    let root = tempdir().expect("temp dir should exist");
    let fix = plan_create_env_example(
        root.path(),
        root.path(),
        &["AUTH_TOKEN".to_string(), "API_URL".to_string()],
    );

    let applied = apply_fix(&fix).expect("create fix should apply");
    let output = stdfs::read_to_string(root.path().join(".env.example"))
        .expect("example file should exist");

    assert_eq!(output, "API_URL=\nAUTH_TOKEN=\n");
    assert_eq!(applied.outcome, "created");
    assert_eq!(applied.id, fix.public.id);
}

#[test]
fn sync_env_example_appends_missing_keys_with_js_newline_behavior() {
    let root = tempdir().expect("temp dir should exist");
    let example_path = root.path().join(".env.example");
    stdfs::write(&example_path, "EXISTING=\n").expect("example file should write");

    let fix = plan_sync_env_example(
        root.path(),
        &example_path,
        "EXISTING=\n",
        &["AUTH_TOKEN".to_string(), "API_URL".to_string()],
    );

    let applied = apply_fix(&fix).expect("sync fix should apply");
    let output = stdfs::read_to_string(&example_path).expect("example file should exist");

    assert_eq!(output, "EXISTING=\nAPI_URL=\nAUTH_TOKEN=\n");
    assert_eq!(applied.outcome, "updated");
    assert_eq!(applied.id, fix.public.id);
}

#[test]
fn sync_env_example_inserts_separator_when_existing_text_has_no_trailing_newline() {
    let root = tempdir().expect("temp dir should exist");
    let example_path = root.path().join("packages/app/.env.example");
    stdfs::create_dir_all(example_path.parent().expect("parent should exist"))
        .expect("parent should be created");

    let fix = plan_sync_env_example(
        root.path(),
        &example_path,
        "EXISTING=",
        &["AUTH_TOKEN".to_string()],
    );

    apply_fix(&fix).expect("sync fix should apply");
    let output = stdfs::read_to_string(&example_path).expect("example file should exist");

    assert_eq!(output, "EXISTING=\nAUTH_TOKEN=\n");
    assert_eq!(fix.public.title, "Append missing keys to packages/app/.env.example");
}

#[test]
fn apply_fixes_runs_multiple_plans_in_order() {
    let root = tempdir().expect("temp dir should exist");
    let create = plan_create_env_example(root.path(), root.path(), &["API_URL".to_string()]);
    let sync = plan_sync_env_example(
        root.path(),
        &root.path().join(".env.example"),
        "API_URL=\n",
        &["AUTH_TOKEN".to_string()],
    );

    let applied = apply_fixes(&[create, sync]).expect("fixes should apply");
    let output = stdfs::read_to_string(root.path().join(".env.example"))
        .expect("example file should exist");

    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].outcome, "created");
    assert_eq!(applied[1].outcome, "updated");
    assert_eq!(output, "API_URL=\nAUTH_TOKEN=\n");
}
