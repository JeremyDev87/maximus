use maximus_core::{make_finding, FileKind, FindingInput, ProjectSnapshot, Severity};

use crate::check_outcome::CheckOutcome;
use crate::registry::{has_object_key, package_file_for_directory, read_package_json};

pub fn run_config_duplicate_check(project: &ProjectSnapshot) -> std::io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let package_file = package_file_for_directory(directory).cloned();
        let package_json = package_file
            .as_ref()
            .and_then(|file| read_package_json(&file.path));

        for (label, file_kind, package_field) in [
            ("ESLint", FileKind::Eslint, "eslintConfig"),
            ("Prettier", FileKind::Prettier, "prettier"),
            ("Jest", FileKind::Jest, "jest"),
        ] {
            let family_files = directory.files_by_kind.get(&file_kind);
            let family_file_count = family_files.map(|files| files.len()).unwrap_or(0);
            let has_package_field = package_json
                .as_ref()
                .map(|value| has_object_key(value, package_field))
                .unwrap_or(false);
            let total_sources = family_file_count + usize::from(has_package_field);

            if total_sources <= 1 {
                continue;
            }

            let severity = if label == "ESLint" {
                Severity::Error
            } else {
                Severity::Warn
            };
            let file = package_file
                .as_ref()
                .map(|value| value.path.clone())
                .or_else(|| family_files.and_then(|files| files.first().map(|file| file.path.clone())));

            findings.push(make_finding(FindingInput {
                id: format!("duplicate-config:{label}:{}", directory.dir.to_string_lossy()),
                title: format!("{label} config is declared in multiple places"),
                category: Some("duplicates".to_string()),
                detail: Some(format!(
                    "Found {total_sources} {label} config sources in {}.",
                    js_detail_directory(&directory.relative_dir)
                )),
                file,
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(format!(
                    "Keep a single {label} entry point per directory to avoid drift."
                )),
                severity: Some(severity),
            }));
        }

        let eslint_files = directory
            .files_by_kind
            .get(&FileKind::Eslint)
            .cloned()
            .unwrap_or_default();
        let has_legacy_eslint = eslint_files
            .iter()
            .any(|file| file.name.starts_with(".eslintrc"));
        let has_flat_eslint = eslint_files
            .iter()
            .any(|file| file.name.starts_with("eslint.config."));

        if has_legacy_eslint && has_flat_eslint {
            findings.push(make_finding(FindingInput {
                id: format!("eslint-mixed-modes:{}", directory.dir.to_string_lossy()),
                title: "Legacy and flat ESLint configs coexist".to_string(),
                category: Some("duplicates".to_string()),
                detail: Some(
                    "ESLint may resolve different config systems depending on invocation and toolchain."
                        .to_string(),
                ),
                file: eslint_files.first().map(|file| file.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Pick either flat config (eslint.config.*) or legacy .eslintrc.* in the same directory."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn js_detail_directory(relative_dir: &str) -> String {
    if cfg!(windows) {
        relative_dir.replace('/', "\\")
    } else {
        relative_dir.to_string()
    }
}
