use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::config::ConfigSuppression;
use maximus_core::findings::summarize_findings_with_suppressed_by_config;
use maximus_core::{
    discover_project, discover_project_with_ignore_root, parse_jsonc, read_text_if_exists,
    sort_findings, unique_fixes, AuditResult, CheckFilterConfig, ConfigSeverity, MaximusConfig,
    PlannedFix, ProjectDirectory, ProjectFile, ProjectSnapshot, Severity,
};
use maximus_core::{is_concrete_env_file_name, is_template_env_file_name};
use serde_json::{Map, Value};

use crate::check_outcome::CheckOutcome;
use crate::editorconfig_prettier::run_editorconfig_prettier_check_with_ignore_root;
use crate::ignore_file_drift::run_ignore_file_drift_check_with_ignore_root;
use crate::jsx_config::run_jsx_config_check;
use crate::lockfiles::run_lockfiles_check_with_ignore_root;
use crate::module_system::run_module_system_check;
use crate::monorepo_tsconfig::run_monorepo_tsconfig_check;
use crate::node_matrix::run_node_matrix_check_with_ignore_root;
use crate::npmignore_files::run_npmignore_files_check_with_ignore_root;
use crate::package_entrypoints::run_package_entrypoints_check;
use crate::test_runner_config::run_test_runner_config_check_with_ignore_root;
use crate::vite_tsconfig_alias::run_vite_tsconfig_alias_check;
use crate::workspace_config::run_workspace_config_check;
use crate::{
    build_structure_report, run_config_duplicate_check, run_eslint_prettier_check,
    run_tsconfig_check, EnvCheckOptions,
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
        id: "module-system",
        run: run_module_system_check_registered,
    },
    RegisteredCheck {
        id: "monorepo-tsconfig",
        run: run_monorepo_tsconfig_check_registered,
    },
    RegisteredCheck {
        id: "jsx-config",
        run: run_jsx_config_check_registered,
    },
    RegisteredCheck {
        id: "lockfiles",
        run: run_lockfiles_check_registered,
    },
    RegisteredCheck {
        id: "package-entrypoints",
        run: run_package_entrypoints_check_registered,
    },
    RegisteredCheck {
        id: "vite-tsconfig-alias",
        run: run_vite_tsconfig_alias_check_registered,
    },
    RegisteredCheck {
        id: "workspace-config",
        run: run_workspace_config_check_registered,
    },
    RegisteredCheck {
        id: "test-runner-config",
        run: run_test_runner_config_check_registered,
    },
    RegisteredCheck {
        id: "editorconfig-prettier",
        run: run_editorconfig_prettier_check_registered,
    },
    RegisteredCheck {
        id: "ignore-drift",
        run: run_ignore_file_drift_check_registered,
    },
    RegisteredCheck {
        id: "node-matrix",
        run: run_node_matrix_check_registered,
    },
    RegisteredCheck {
        id: "npmignore-files",
        run: run_npmignore_files_check_registered,
    },
];

pub fn registered_check_ids() -> &'static [&'static str] {
    &[
        "duplicates",
        "env",
        "eslint-prettier",
        "tsconfig",
        "module-system",
        "monorepo-tsconfig",
        "jsx-config",
        "lockfiles",
        "package-entrypoints",
        "vite-tsconfig-alias",
        "workspace-config",
        "test-runner-config",
        "editorconfig-prettier",
        "ignore-drift",
        "node-matrix",
        "npmignore-files",
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
    let ignored_patterns = config.effective_ignore_patterns();
    let project = if ignored_patterns.is_empty() {
        discover_project(root_dir)?
    } else {
        discover_project_with_ignore_root(root_dir, &ignored_patterns, ignore_root)?
    };
    let mut outcome = run_registered_checks_with_config_root(&project, config, ignore_root)?;
    apply_severity_overrides(&mut outcome.findings, &config.severity);
    let suppressed_by_config = apply_config_suppressions(
        &mut outcome,
        &config.suppressions,
        &project.root_dir,
        ignore_root,
    );
    outcome.findings = sort_findings(&outcome.findings);
    let structure = build_structure_report(&project, &outcome.findings);
    let summary = summarize_findings_with_suppressed_by_config(
        &outcome.findings,
        &outcome.fixes,
        &structure,
        suppressed_by_config,
    );
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

pub(crate) fn tsconfig_entry_file_for_directory(
    directory: &ProjectDirectory,
) -> Option<&ProjectFile> {
    directory
        .files_by_kind
        .get(&maximus_core::FileKind::Tsconfig)
        .and_then(|files| {
            files
                .iter()
                .find(|file| file.name == "tsconfig.json" || file.name == "jsconfig.json")
        })
}

pub(crate) fn read_package_json(file_path: &Path) -> Option<serde_json::Value> {
    let text = read_text_if_exists(file_path).ok().flatten()?;
    parse_jsonc::<serde_json::Value>(&text, &file_path.to_string_lossy()).ok()
}

pub(crate) fn read_effective_compiler_options(
    file_path: &Path,
) -> io::Result<Option<Map<String, Value>>> {
    let Some(config) = read_tsconfig_json(file_path)? else {
        return Ok(None);
    };
    let mut visited = HashSet::new();
    read_effective_compiler_options_inner(file_path, &config, &mut visited)
}

fn read_effective_compiler_options_inner(
    config_path: &Path,
    config: &Value,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<Option<Map<String, Value>>> {
    let normalized_path = normalize_tsconfig_path(config_path);
    if !visited.insert(normalized_path) {
        return Ok(None);
    }

    let mut compiler_options = match load_extended_tsconfig_document(config_path, config, visited)?
    {
        Some((parent_config_path, parent_config)) => {
            read_effective_compiler_options_inner(&parent_config_path, &parent_config, visited)?
                .unwrap_or_default()
        }
        None => Map::new(),
    };

    if let Some(config_options) = config.get("compilerOptions").and_then(Value::as_object) {
        for (key, value) in config_options {
            compiler_options.insert(key.clone(), value.clone());
        }
    }

    Ok(Some(compiler_options))
}

fn read_tsconfig_json(file_path: &Path) -> io::Result<Option<Value>> {
    let Some(text) = read_text_if_exists(file_path)? else {
        return Ok(None);
    };

    Ok(parse_jsonc::<Value>(&text, &file_path.to_string_lossy()).ok())
}

fn load_extended_tsconfig_document(
    config_path: &Path,
    config: &Value,
    visited: &HashSet<PathBuf>,
) -> io::Result<Option<(PathBuf, Value)>> {
    let Some(extends_path) = config.get("extends").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(parent_config_path) = resolve_extends_config_path(
        config_path.parent().unwrap_or_else(|| Path::new(".")),
        extends_path,
    ) else {
        return Ok(None);
    };

    if visited.contains(&normalize_tsconfig_path(&parent_config_path)) {
        return Ok(None);
    }

    let Some(parent_config) = read_tsconfig_json(&parent_config_path)? else {
        return Ok(None);
    };

    Ok(Some((parent_config_path, parent_config)))
}

fn resolve_extends_config_path(base_dir: &Path, extends_path: &str) -> Option<PathBuf> {
    let extends_candidate = Path::new(extends_path);
    let is_local_extends = extends_candidate.is_absolute()
        || extends_path.starts_with("./")
        || extends_path.starts_with("../")
        || extends_path.starts_with(".\\")
        || extends_path.starts_with("..\\");

    if is_local_extends {
        return resolve_tsconfig_candidate(&base_dir.join(extends_path.replace('\\', "/")))
            .ok()
            .flatten();
    }

    for ancestor in base_dir.ancestors() {
        let candidate = ancestor.join("node_modules").join(extends_path);
        if let Ok(Some(resolved)) = resolve_tsconfig_candidate(&candidate) {
            return Some(resolved);
        }
    }

    None
}

fn resolve_tsconfig_candidate(candidate: &Path) -> io::Result<Option<PathBuf>> {
    if candidate.exists() {
        let metadata = fs::metadata(candidate)?;
        if metadata.is_dir() {
            let directory_target = candidate.join("tsconfig.json");
            if directory_target.exists() {
                return Ok(Some(directory_target));
            }
            return Ok(None);
        }

        return Ok(Some(candidate.to_path_buf()));
    }

    if candidate.extension().is_none() {
        let file_target = candidate.with_extension("json");
        if file_target.exists() {
            return Ok(Some(file_target));
        }
    }

    let directory_target = candidate.join("tsconfig.json");
    if directory_target.exists() {
        return Ok(Some(directory_target));
    }

    Ok(None)
}

fn normalize_tsconfig_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_env_check_with_config_root_and_options(
        project,
        config,
        ignore_root,
        &EnvCheckOptions::default(),
    )
}

pub fn run_env_check_with_config_root_and_options(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
    options: &EnvCheckOptions,
) -> io::Result<CheckOutcome> {
    if !config.gitignore_patterns.is_empty() {
        let ignored_patterns = env_rediscovery_ignore_patterns(config);
        let env_project = if ignored_patterns.is_empty() {
            discover_project(&project.root_dir)?
        } else {
            discover_project_with_ignore_root(&project.root_dir, &ignored_patterns, ignore_root)?
        };
        return crate::env::run_env_check_with_missing_concrete_excluded_keys(
            &env_project,
            options,
            &config.env.missing_concrete_excluded_keys(),
        );
    }

    crate::env::run_env_check_with_missing_concrete_excluded_keys(
        project,
        options,
        &config.env.missing_concrete_excluded_keys(),
    )
}

fn run_ignore_file_drift_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.effective_ignore_patterns();
    run_ignore_file_drift_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn run_node_matrix_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.effective_ignore_patterns();
    run_node_matrix_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn run_npmignore_files_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.non_git_ignore_patterns();
    run_npmignore_files_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn env_rediscovery_ignore_patterns(config: &MaximusConfig) -> Vec<String> {
    let mut patterns = config
        .gitignore_patterns
        .iter()
        .filter_map(|pattern| env_rediscovery_gitignore_pattern(pattern))
        .collect::<Vec<_>>();
    patterns.extend(config.non_git_ignore_patterns());
    patterns
}

fn env_rediscovery_gitignore_pattern(pattern: &str) -> Option<String> {
    if is_env_file_name_ignore_pattern(pattern) {
        Some(format!("{}/", pattern.trim_end().trim_end_matches('/')))
    } else {
        Some(pattern.to_string())
    }
}

fn is_env_file_name_ignore_pattern(pattern: &str) -> bool {
    let pattern = pattern.trim_end();
    if pattern.starts_with('!') {
        return false;
    }
    if pattern.ends_with('/') {
        return false;
    }

    let pattern = pattern.trim_start_matches('/');
    let file_pattern = pattern.rsplit('/').next().unwrap_or(pattern);
    if file_pattern.contains('*') || file_pattern.contains('?') {
        return is_env_specific_glob_pattern(file_pattern)
            && glob_pattern_can_match_env_file_name(file_pattern);
    }

    is_concrete_env_file_name(file_pattern) || is_template_env_file_name(file_pattern)
}

fn is_env_specific_glob_pattern(pattern: &str) -> bool {
    pattern.starts_with(".env") || pattern.starts_with("*.env")
}

fn glob_pattern_can_match_env_file_name(pattern: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; 7]; pattern.len() + 1];
    env_name_glob_intersects(&pattern, 0, 0, &mut memo)
}

fn env_name_glob_intersects(
    pattern: &[char],
    pattern_index: usize,
    env_state: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if pattern_index == pattern.len() {
        return env_name_state_accepts(env_state);
    }
    if let Some(result) = memo[pattern_index][env_state] {
        return result;
    }

    let result = match pattern[pattern_index] {
        '*' => env_name_wildcard_closure(env_state)
            .iter()
            .enumerate()
            .filter(|(_, reachable)| **reachable)
            .any(|(state, _)| env_name_glob_intersects(pattern, pattern_index + 1, state, memo)),
        '?' => env_name_wildcard_next_states(env_state)
            .iter()
            .any(|state| env_name_glob_intersects(pattern, pattern_index + 1, *state, memo)),
        character => env_name_next_state(env_state, character)
            .is_some_and(|state| env_name_glob_intersects(pattern, pattern_index + 1, state, memo)),
    };
    memo[pattern_index][env_state] = Some(result);
    result
}

fn env_name_next_state(state: usize, character: char) -> Option<usize> {
    match (state, character) {
        (0, '.') => Some(1),
        (1, 'e') => Some(2),
        (2, 'n') => Some(3),
        (3, 'v') => Some(4),
        (4, '.') => Some(5),
        (5, '/') | (6, '/') => None,
        (5, _) => Some(6),
        (6, _) => Some(6),
        _ => None,
    }
}

fn env_name_wildcard_next_states(state: usize) -> &'static [usize] {
    match state {
        0 => &[1],
        1 => &[2],
        2 => &[3],
        3 => &[4],
        4 => &[5],
        5 => &[6],
        6 => &[6],
        _ => &[],
    }
}

fn env_name_wildcard_closure(state: usize) -> [bool; 7] {
    let mut reachable = [false; 7];
    let mut stack = vec![state];
    reachable[state] = true;

    while let Some(state) = stack.pop() {
        for next_state in env_name_wildcard_next_states(state) {
            if !reachable[*next_state] {
                reachable[*next_state] = true;
                stack.push(*next_state);
            }
        }
    }

    reachable
}

fn env_name_state_accepts(state: usize) -> bool {
    matches!(state, 4 | 6)
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

fn run_module_system_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_module_system_check(project)
}

fn run_monorepo_tsconfig_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_monorepo_tsconfig_check(project)
}

fn run_jsx_config_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_jsx_config_check(project)
}

fn run_lockfiles_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.effective_ignore_patterns();
    run_lockfiles_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn run_package_entrypoints_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_package_entrypoints_check(project)
}

fn run_vite_tsconfig_alias_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_vite_tsconfig_alias_check(project)
}

fn run_workspace_config_check_registered(
    project: &ProjectSnapshot,
    _config: &MaximusConfig,
    _ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    run_workspace_config_check(project)
}

fn run_test_runner_config_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.effective_ignore_patterns();
    run_test_runner_config_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn run_editorconfig_prettier_check_registered(
    project: &ProjectSnapshot,
    config: &MaximusConfig,
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let ignored_patterns = config.effective_ignore_patterns();
    run_editorconfig_prettier_check_with_ignore_root(project, &ignored_patterns, ignore_root)
}

fn apply_config_suppressions(
    outcome: &mut CheckOutcome,
    suppressions: &[ConfigSuppression],
    root_dir: &Path,
    ignore_root: &Path,
) -> usize {
    if suppressions.is_empty() || outcome.findings.is_empty() {
        return 0;
    }

    let original_count = outcome.findings.len();
    outcome
        .findings
        .retain(|finding| !is_suppressed_by_config(finding, suppressions, root_dir, ignore_root));
    let suppressed_count = original_count - outcome.findings.len();

    if suppressed_count > 0 {
        let active_fix_ids = outcome
            .findings
            .iter()
            .flat_map(|finding| finding.fix_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        outcome.fixes.retain(|fix| active_fix_ids.contains(&fix.id));
        outcome
            .planned_fixes
            .retain(|fix| active_fix_ids.contains(&fix.public.id));
    }

    suppressed_count
}

fn is_suppressed_by_config(
    finding: &maximus_core::Finding,
    suppressions: &[ConfigSuppression],
    root_dir: &Path,
    ignore_root: &Path,
) -> bool {
    suppressions.iter().any(|suppression| {
        suppression.id == finding.id
            && suppression_file_matches(finding, suppression, root_dir, ignore_root)
    })
}

fn suppression_file_matches(
    finding: &maximus_core::Finding,
    suppression: &ConfigSuppression,
    root_dir: &Path,
    ignore_root: &Path,
) -> bool {
    let Some(prefix) = suppression
        .file_prefix
        .as_deref()
        .and_then(normalize_file_prefix)
    else {
        return true;
    };
    let Some(file) = finding.file.as_ref() else {
        return false;
    };

    finding_file_candidates(file, &[root_dir, ignore_root])
        .iter()
        .any(|candidate| path_matches_prefix(candidate, &prefix))
}

fn finding_file_candidates(file: &Path, roots: &[&Path]) -> Vec<String> {
    let mut candidates = vec![path_to_slash_string(file)];
    if let Ok(canonical_file) = fs::canonicalize(file) {
        push_unique_candidate(&mut candidates, path_to_slash_string(&canonical_file));
    }

    for root in roots {
        push_relative_candidate(&mut candidates, file, root);
    }

    candidates
}

fn push_relative_candidate(candidates: &mut Vec<String>, file: &Path, root: &Path) {
    if let Ok(relative) = file.strip_prefix(root) {
        push_unique_candidate(candidates, path_to_slash_string(relative));
    }

    if let (Ok(canonical_file), Ok(canonical_root)) =
        (fs::canonicalize(file), fs::canonicalize(root))
    {
        if let Ok(relative) = canonical_file.strip_prefix(canonical_root) {
            push_unique_candidate(candidates, path_to_slash_string(relative));
        }
    }
}

fn push_unique_candidate(candidates: &mut Vec<String>, candidate: String) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn normalize_file_prefix(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
