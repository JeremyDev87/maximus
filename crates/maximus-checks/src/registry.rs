use std::io;
use std::path::Path;

use maximus_core::{
    discover_project, parse_jsonc, sort_findings, summarize_findings, unique_fixes, AuditResult,
    PlannedFix, ProjectDirectory, ProjectFile, ProjectSnapshot, read_text_if_exists,
};

use crate::check_outcome::CheckOutcome;
use crate::{
    build_structure_report, run_config_duplicate_check, run_env_check, run_eslint_prettier_check,
    run_tsconfig_check,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditedProject {
    pub project: ProjectSnapshot,
    pub result: AuditResult,
    pub planned_fixes: Vec<PlannedFix>,
}

pub fn run_registered_checks(project: &ProjectSnapshot) -> std::io::Result<CheckOutcome> {
    let outcomes = [
        run_config_duplicate_check(project)?,
        run_env_check(project)?,
        run_eslint_prettier_check(project)?,
        run_tsconfig_check(project)?,
    ];

    Ok(merge_outcomes(outcomes))
}

pub fn audit_project(root_dir: &Path) -> io::Result<AuditedProject> {
    let project = discover_project(root_dir)?;
    let outcome = run_registered_checks(&project)?;
    let structure = build_structure_report(&project, &outcome.findings);
    let summary = summarize_findings(&outcome.findings, &outcome.fixes, &structure);
    let result = AuditResult {
        root_dir: project.root_dir.clone(),
        summary,
        structure,
        findings: outcome.findings,
        fixes: outcome.fixes,
    };

    Ok(AuditedProject {
        project,
        result,
        planned_fixes: outcome.planned_fixes,
    })
}

pub(crate) fn merge_outcomes<I>(outcomes: I) -> CheckOutcome
where
    I: IntoIterator<Item = CheckOutcome>,
{
    let mut findings = Vec::new();
    let mut fixes = Vec::new();
    let mut planned_fixes = Vec::new();

    for outcome in outcomes {
        findings.extend(outcome.findings);
        fixes.extend(outcome.fixes);
        planned_fixes.extend(outcome.planned_fixes);
    }

    CheckOutcome {
        findings: sort_findings(&findings),
        fixes: unique_fixes(&fixes),
        planned_fixes: unique_planned_fixes(&planned_fixes),
    }
}

fn unique_planned_fixes(fixes: &[PlannedFix]) -> Vec<PlannedFix> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();

    for fix in fixes {
        if seen.insert(fix.public.id.clone()) {
            unique.push(fix.clone());
        }
    }

    unique
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
