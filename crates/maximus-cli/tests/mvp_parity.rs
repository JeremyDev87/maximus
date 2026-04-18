use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::tempdir;

fn rust_maximus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maximus"))
}

fn js_maximus() -> Command {
    let mut command = Command::new("node");
    command.arg(workspace_root().join("bin/maximus.js"));
    command
}

#[test]
fn text_commands_match_js_reference_outputs() {
    for args in [
        vec!["audit".to_string(), fixture_path("clean-project").display().to_string()],
        vec!["doctor".to_string(), fixture_path("clean-project").display().to_string()],
        vec!["audit".to_string(), fixture_path("reference-env").display().to_string()],
        vec!["doctor".to_string(), fixture_path("reference-env").display().to_string()],
        vec![
            "fix".to_string(),
            fixture_path("reference-env").display().to_string(),
            "--dry-run".to_string(),
        ],
        vec![
            "audit".to_string(),
            fixture_path("reference-tsconfig").display().to_string(),
        ],
    ] {
        assert_command_matches_js(&args);
    }
}

#[test]
fn json_commands_match_js_reference_outputs() {
    for args in [
        vec![
            "audit".to_string(),
            fixture_path("clean-project").display().to_string(),
            "--json".to_string(),
        ],
        vec![
            "doctor".to_string(),
            fixture_path("reference-env").display().to_string(),
            "--json".to_string(),
        ],
        vec![
            "audit".to_string(),
            fixture_path("reference-tsconfig").display().to_string(),
            "--json".to_string(),
        ],
        vec![
            "fix".to_string(),
            fixture_path("reference-env").display().to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
        ],
    ] {
        assert_json_command_matches_js(&args);
    }
}

#[test]
fn fix_apply_matches_js_for_text_output_and_written_file() {
    let rust_fixture = prepare_temp_reference_env();
    let js_fixture = prepare_temp_reference_env();

    let rust_output = rust_maximus()
        .args(["fix", rust_fixture.to_string_lossy().as_ref()])
        .output()
        .expect("rust fix should run");
    let js_output = js_maximus()
        .args(["fix", js_fixture.to_string_lossy().as_ref()])
        .output()
        .expect("js fix should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());
    assert_eq!(
        normalize_output(&rust_output.stdout, &rust_fixture),
        normalize_output(&js_output.stdout, &js_fixture)
    );
    assert_eq!(rust_output.stderr, js_output.stderr);
    assert_eq!(
        fs::read_to_string(rust_fixture.join(".env.example")).expect("rust output file should exist"),
        fs::read_to_string(js_fixture.join(".env.example")).expect("js output file should exist")
    );
}

#[test]
fn fix_apply_matches_js_for_json_shape_and_written_file() {
    let rust_fixture = prepare_temp_reference_env();
    let js_fixture = prepare_temp_reference_env();

    let rust_output = rust_maximus()
        .args(["fix", rust_fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("rust fix json should run");
    let js_output = js_maximus()
        .args(["fix", js_fixture.to_string_lossy().as_ref(), "--json"])
        .output()
        .expect("js fix json should run");

    assert_eq!(rust_output.status.code(), js_output.status.code());
    assert_eq!(rust_output.stderr, js_output.stderr);

    let rust_json = normalize_json_output(&rust_output, &rust_fixture);
    let js_json = normalize_json_output(&js_output, &js_fixture);

    assert_eq!(rust_json, js_json);
    assert_eq!(
        fs::read_to_string(rust_fixture.join(".env.example")).expect("rust output file should exist"),
        fs::read_to_string(js_fixture.join(".env.example")).expect("js output file should exist")
    );
}

fn assert_command_matches_js(args: &[String]) {
    let rust_output = rust_maximus()
        .args(args)
        .output()
        .expect("rust command should run");
    let js_output = js_maximus()
        .args(args)
        .output()
        .expect("js command should run");

    assert_eq!(rust_output.status.code(), js_output.status.code(), "{args:?}");
    assert_eq!(
        normalize_output_for_args(&rust_output, args),
        normalize_output_for_args(&js_output, args),
        "{args:?}"
    );
    assert_eq!(rust_output.stderr, js_output.stderr, "{args:?}");
}

fn assert_json_command_matches_js(args: &[String]) {
    let rust_output = rust_maximus()
        .args(args)
        .output()
        .expect("rust command should run");
    let js_output = js_maximus()
        .args(args)
        .output()
        .expect("js command should run");

    assert_eq!(rust_output.status.code(), js_output.status.code(), "{args:?}");
    assert_eq!(
        normalize_json_output(&rust_output, Path::new(&args[1])),
        normalize_json_output(&js_output, Path::new(&args[1])),
        "{args:?}"
    );
    assert_eq!(rust_output.stderr, js_output.stderr, "{args:?}");
}

fn normalize_output_for_args(output: &Output, args: &[String]) -> String {
    normalize_output(&output.stdout, Path::new(&args[1]))
}

fn normalize_output(stdout: &[u8], target_dir: &Path) -> String {
    let target = target_dir.to_string_lossy().into_owned();
    String::from_utf8(stdout.to_vec())
        .expect("stdout should be utf8")
        .replace("\r\n", "\n")
        .replace(&target, "<TARGET>")
}

fn normalize_json_output(output: &Output, target_dir: &Path) -> Value {
    let mut value: Value = serde_json::from_slice(&output.stdout).expect("json output should parse");
    rewrite_json_paths(&mut value, target_dir);
    value
}

fn rewrite_json_paths(value: &mut Value, target_dir: &Path) {
    let target = target_dir.to_string_lossy().into_owned();
    match value {
        Value::String(text) => {
            *text = text.replace(&target, "<TARGET>");
        }
        Value::Array(values) => {
            for entry in values {
                rewrite_json_paths(entry, Path::new(&target));
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                rewrite_json_paths(value, Path::new(&target));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn fixture_path(name: &str) -> PathBuf {
    workspace_root().join("test/fixtures").join(name)
}

fn prepare_temp_reference_env() -> PathBuf {
    let temp = tempdir().expect("temp dir should exist");
    let target = temp.keep();
    let source = fixture_path("reference-env").join(".env");

    fs::create_dir_all(&target).expect("target dir should exist");
    fs::copy(source, target.join(".env")).expect("env fixture should copy");

    target
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}
