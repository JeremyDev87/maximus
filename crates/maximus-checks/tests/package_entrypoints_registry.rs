use std::fs;
use std::path::Path;

use maximus_checks::audit_project;
use tempfile::TempDir;

#[test]
fn audit_project_runs_package_entrypoints_check() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("package.json"),
        r#"{ "main": "./dist/missing-main.js" }"#,
    );

    let audited = audit_project(fixture.path()).expect("audit should run");

    assert!(
        audited
            .result
            .findings
            .iter()
            .any(|finding| finding.id
                == format!(
                    "package-entrypoints:{}:main:./dist/missing-main.js",
                    fixture.path().join("package.json").to_string_lossy()
                )),
        "registered audit path should include package-entrypoints findings"
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}
