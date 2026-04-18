use std::path::Path;

use maximus_core::{
    parse_jsonc, sort_findings, unique_fixes, FixPlan, Finding, ProjectDirectory, ProjectFile,
    ProjectSnapshot, read_text_if_exists,
};

use crate::{run_config_duplicate_check, run_eslint_prettier_check};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOutcome {
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
}

impl CheckOutcome {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub fn run_registered_checks(project: &ProjectSnapshot) -> std::io::Result<CheckOutcome> {
    let outcomes = [
        run_config_duplicate_check(project)?,
        run_eslint_prettier_check(project)?,
    ];

    Ok(merge_outcomes(outcomes))
}

pub(crate) fn merge_outcomes<I>(outcomes: I) -> CheckOutcome
where
    I: IntoIterator<Item = CheckOutcome>,
{
    let mut findings = Vec::new();
    let mut fixes = Vec::new();

    for outcome in outcomes {
        findings.extend(outcome.findings);
        fixes.extend(outcome.fixes);
    }

    CheckOutcome {
        findings: sort_findings(&findings),
        fixes: unique_fixes(&fixes),
    }
}

pub(crate) fn package_file_for_directory(directory: &ProjectDirectory) -> Option<&ProjectFile> {
    directory.files.iter().find(|file| file.kind == maximus_core::FileKind::Package)
}

pub(crate) fn read_package_json(file_path: &Path) -> Option<serde_json::Value> {
    let text = read_text_if_exists(file_path).ok().flatten()?;
    parse_jsonc::<serde_json::Value>(&text, &file_path.to_string_lossy()).ok()
}

pub(crate) fn has_object_key(value: &serde_json::Value, key: &str) -> bool {
    value
        .as_object()
        .map(|object| object.contains_key(key))
        .unwrap_or(false)
}
