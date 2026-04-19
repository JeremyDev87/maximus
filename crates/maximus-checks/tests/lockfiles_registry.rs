use std::fs;
use std::path::Path;

use maximus_checks::audit_project;
use tempfile::TempDir;

#[test]
fn audit_project_runs_lockfiles_check() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(fixture.path().join("package-lock.json"), "{}\n");
    write(fixture.path().join("yarn.lock"), "# yarn lockfile v1\n");

    let audited = audit_project(fixture.path()).expect("audit should run");

    assert!(
        audited
            .result
            .findings
            .iter()
            .any(|finding| {
                finding.id
                    == format!("lockfiles:multiple:{}", fixture.path().to_string_lossy())
            }),
        "registered audit path should include lockfiles findings"
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}
