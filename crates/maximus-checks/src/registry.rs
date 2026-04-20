use std::io;
use std::path::Path;

use maximus_core::{
    discover_project, discover_project_with_ignore_root, parse_jsonc, read_text_if_exists,
    sort_findings, summarize_findings, unique_fixes, AuditResult, CheckFilterConfig,
    ConfigSeverity, MaximusConfig, PlannedFix, ProjectDirectory, ProjectFile, ProjectSnapshot,
    Severity,
};

use crate::check_outcome::CheckOutcome;
use crate::lockfiles::run_lockfiles_check_with_ignore_root;
use crate::package_entrypoints::run_package_entrypoints_check;
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

type RegisteredCheckFn = fn(&ProjectSnapshot, &MaximusConfig, &Path) -> io::Result<CheckOutcome>;

struct RegisteredCheck {
    id: &'static str,
    run: RegisteredCheckFn,
}

const REGISTERED_CHECKS: &[RegisteredCheck] = &[
    RegisteredCheck {
        id: "duplicates",
        run: run_config_duplicate_check_registered,
    },
    RegisteredCheck {
        id: "env",
        run: run_env_check_registered,
    },
    RegisteredCheck {
        id: "eslint-prettier",
        run: run_eslint_prettier_check_registered,
    },
    RegisteredCheck {
        id: "tsconfig",
        run: run_tsconfig_check_registered,
    },
    RegisteredCheck {
        id: "lockfiles",
        run: run_lockfiles_check_registered,
    },
    RegisteredCheck {
        id: "package-entrypoints",
        run: run_package_entrypoints_check_registered,
    },
];

pub fn registered_check_ids() -> &'static [&'static str] {
    &[
        "duplicates",
        "env",
        "eslint-prettier",
        "tsconfig",
        "lockfiles",
        "package-entrypoints",
    ]
}

pub fn run_registered_checks(project: &ProjectSnapshot) -> std::io::Result<CheckOutcome> {
    run_registered_checks_with_config_root(project, &MaximusConfig::default(), &project.root_dir)
}

pub fn run_registered_checks_with_filters(
    project: &ProjectSnapshot,
    filters: &CheckFilterConfig,
) -> std::io::Result<CheckOutcome> {
    let config = MaximusConfig {
        checks: filters.clone(),
        ..MaximusConfig::default()
    };
    run_registered_checks_with_config_root(project, &config, &project.root_dir)
}

pub fn run_registered_checks_with_config(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
) -> std::io::Result<CheckOutcome> {
    run_registered_checks_with_config_root(project, config, &project.root_dir)
}

pub fn run_registered_checks_with_config_root(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> std::io::Result<CheckOutcome> {
    let outcomes = REGISTERED_CHECKS
        .iter()
        .filter(|check| should_run_check(check.id, &config.checks))
        .map(|check| (check.run)(project, config, ignore_root))
        .collect::<io::Result<Vec<_>>>()?;

    Ok(merge_outcomes(outcomes))
}

pub fn audit_project(root_dir: &Path) -> io::Result<AuditedProject> {
    audit_project_with_config(root_dir, &MaximusConfig::default())
}

pub fn audit_project_with_config(
    root_dir: &Path,
    config: &MaximusConfig,
) -> io::Result<AuditedProject> {
    audit_project_with_config_root(root_dir, config, root_dir)
}

pub fn audit_project_with_config_root(
    root_dir: &Path,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<AuditedProject> {
    let project = if config.ignore.is_empty() {
        discover_project(root_dir)?
    } else {
        discover_project_with_ignore_root(root_dir, &config.ignore, ignore_root)?
    };
    let mut outcome = run_registered_checks_with_config_root(&project, config, ignore_root)?;
    apply_severity_overrides(&mut outcome.findings, &config.severity);
    outcome.findings = sort_findings(&outcome.findings);
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
    directory
        .files
        .iter()
        .find(|file| file.kind == maximus_core::FileKind::Package)
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

fn should_run_check(id: &str, filters: &CheckFilterConfig) -> bool {
    let allowed = filters.only.is_empty() || filters.only.iter().any(|candidate| candidate == id);
    let skipped = filters.skip.iter().any(|candidate| candidate == id);

    allowed && !skipped
}

fn run_config_duplicate_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_config_duplicate_check(project)
}

fn run_env_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_env_check(project)
}

fn run_eslint_prettier_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_eslint_prettier_check(project)
}

fn run_tsconfig_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_tsconfig_check(project)
}

fn run_lockfiles_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_lockfiles_check_with_ignore_root(project, &config.ignore, ignore_root)
}

fn run_package_entrypoints_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_package_entrypoints_check(project)
}

fn apply_severity_overrides(
    findings: &mut [maximus_core::Finding],
    overrides: &std::collections::BTreeMap<String, ConfigSeverity>,
) {
    if overrides.is_empty() {
        return;
    }

    for finding in findings {
        let override_level = overrides
            .iter()
            .filter(|(prefix, _)| !prefix.trim().is_empty())
            .filter(|(prefix, _)| finding.id.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, level)| level);

        if let Some(level) = override_level {
            finding.severity = config_severity_to_runtime(level);
        }
    }
}

fn config_severity_to_runtime(level: &ConfigSeverity) -> Severity {
    match level {
        ConfigSeverity::Error => Severity::Error,
        ConfigSeverity::Warn => Severity::Warn,
        ConfigSeverity::Info => Severity::Info,
    }
}
