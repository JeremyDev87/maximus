use std::fs;
use std::path::{Path, PathBuf};

use maximus_checks::lockfiles::{
    run_lockfiles_check, run_lockfiles_check_with_ignore, run_lockfiles_check_with_ignore_root,
};
use maximus_core::{discover_project, ProjectSnapshot, Severity};
use tempfile::TempDir;

#[test]
fn lockfiles_check_warns_when_multiple_known_lockfiles_share_a_directory() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(fixture.path().join("package-lock.json"), "{}\n");
    write(fixture.path().join("yarn.lock"), "# yarn lockfile v1\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_lockfiles_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("lockfiles:multiple:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Multiple lockfiles are present in one directory",
        &format!(
            "Found 2 known lockfiles in .: package-lock.json, yarn.lock."
        ),
        "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile.",
        Some(fixture.path().join("package-lock.json")),
    );
}

#[test]
fn lockfiles_check_keeps_root_and_nested_package_directories_independent() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(fixture.path().join("yarn.lock"), "# yarn lockfile v1\n");
    write(
        fixture.path().join("packages/app/package-lock.json"),
        "{}\n",
    );
    write(
        fixture.path().join("packages/app/pnpm-lock.yaml"),
        "lockfileVersion: 9\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_lockfiles_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "lockfiles:multiple:{}",
            fixture.path().join("packages/app").to_string_lossy()
        ),
        Severity::Warn,
        "Multiple lockfiles are present in one directory",
        "Found 2 known lockfiles in packages/app: package-lock.json, pnpm-lock.yaml.",
        "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile.",
        Some(fixture.path().join("packages/app/package-lock.json")),
    );

    assert!(
        outcome.findings.iter().all(|finding| {
            finding.id != format!("lockfiles:multiple:{}", fixture.path().to_string_lossy())
        }),
        "root directory should not inherit the nested package warning"
    );
}

#[test]
fn lockfiles_check_respects_config_ignore_patterns() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(fixture.path().join("ignored/package-lock.json"), "{}\n");
    write(
        fixture.path().join("ignored/yarn.lock"),
        "# yarn lockfile v1\n",
    );
    write(
        fixture.path().join("packages/web/package-lock.json"),
        "{}\n",
    );
    write(
        fixture.path().join("packages/web/pnpm-lock.yaml"),
        "lockfileVersion: 9\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let ignored_patterns = vec!["ignored".to_string()];
    let outcome =
        run_lockfiles_check_with_ignore(&project, &ignored_patterns).expect("check should run");

    assert!(
        outcome
            .findings
            .iter()
            .all(|finding| !finding.id.contains("/ignored")),
        "ignored directory should not produce lockfile findings: {:?}",
        outcome.findings
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "lockfiles:multiple:{}",
            fixture.path().join("packages/web").to_string_lossy()
        ),
        Severity::Warn,
        "Multiple lockfiles are present in one directory",
        "Found 2 known lockfiles in packages/web: package-lock.json, pnpm-lock.yaml.",
        "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile.",
        Some(fixture.path().join("packages/web/package-lock.json")),
    );
}

#[test]
fn lockfiles_check_skips_directly_ignored_target_directory() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let target = fixture.path().join("packages/web/generated");

    write(target.join("package-lock.json"), "{}\n");
    write(target.join("yarn.lock"), "# yarn lockfile v1\n");

    let project = ProjectSnapshot {
        root_dir: target.clone(),
        files: Vec::new(),
        directories: Vec::new(),
        files_by_kind: Default::default(),
        package_files: Vec::new(),
    };
    let ignored_patterns = vec!["packages/web/generated".to_string()];
    let outcome = run_lockfiles_check_with_ignore_root(&project, &ignored_patterns, fixture.path())
        .expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "ignored target directory should not produce lockfile findings: {:?}",
        outcome.findings
    );
}

#[cfg(unix)]
#[test]
fn lockfiles_check_skips_permission_denied_directories() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = TempDir::new().expect("temp dir should exist");
    let blocked = fixture.path().join("blocked");

    write(fixture.path().join("package-lock.json"), "{}\n");
    write(fixture.path().join("yarn.lock"), "# yarn lockfile v1\n");
    fs::create_dir_all(&blocked).expect("blocked dir should exist");
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0))
        .expect("blocked dir should become unreadable");

    let project = ProjectSnapshot {
        root_dir: fixture.path().to_path_buf(),
        files: Vec::new(),
        directories: Vec::new(),
        files_by_kind: Default::default(),
        package_files: Vec::new(),
    };
    let outcome = run_lockfiles_check(&project).expect("check should run");

    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755))
        .expect("blocked dir permissions should restore");

    assert_has_finding(
        &outcome.findings,
        &format!("lockfiles:multiple:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Multiple lockfiles are present in one directory",
        "Found 2 known lockfiles in .: package-lock.json, yarn.lock.",
        "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile.",
        Some(fixture.path().join("package-lock.json")),
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
