use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{
    make_finding, parse_jsonc, read_text_if_exists, FindingInput, ProjectSnapshot, Severity,
};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;
use crate::registry::{package_file_for_directory, tsconfig_entry_file_for_directory};

const SHARED_BASE_TSCONFIG: &str = "tsconfig.base.json";

pub fn run_monorepo_tsconfig_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();
    let root_base_path = project.root_dir.join(SHARED_BASE_TSCONFIG);

    if project.package_files.len() <= 1 || !root_base_path.is_file() {
        return Ok(CheckOutcome {
            findings,
            fixes: Vec::new(),
            planned_fixes: Vec::new(),
        });
    }

    for directory in &project.directories {
        if directory.relative_dir == "." {
            continue;
        }

        if package_file_for_directory(directory).is_none() {
            continue;
        }

        let Some(tsconfig_file) = tsconfig_entry_file_for_directory(directory) else {
            continue;
        };

        let Some(text) = read_text_if_exists(&tsconfig_file.path)? else {
            continue;
        };
        let Ok(config) = parse_jsonc::<Value>(&text, &tsconfig_file.path.to_string_lossy()) else {
            continue;
        };

        if tsconfig_extends_root_base(&tsconfig_file.path, &config, &root_base_path)? {
            continue;
        }

        let extends_description = config
            .get("extends")
            .and_then(Value::as_str)
            .map(|value| format!("It currently extends {value}."))
            .unwrap_or_else(|| "It does not extend a shared base config.".to_string());

        findings.push(make_finding(FindingInput {
            id: format!("monorepo-tsconfig-drift:{}", tsconfig_file.path.to_string_lossy()),
            title: "Package tsconfig drifts from the shared base".to_string(),
            category: Some("monorepo-tsconfig".to_string()),
            detail: Some(format!(
                "{} package config should extend tsconfig.base.json so shared compiler settings stay aligned.",
                extends_description
            )),
            file: Some(tsconfig_file.path.clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Point package-level tsconfig files at the repo root tsconfig.base.json before adding local overrides."
                    .to_string(),
            ),
            severity: Some(Severity::Warn),
        }));
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn tsconfig_extends_root_base(
    config_path: &Path,
    config: &Value,
    root_base_path: &Path,
) -> io::Result<bool> {
    let mut visited = HashSet::new();
    tsconfig_extends_root_base_inner(config_path, config, root_base_path, &mut visited)
}

fn tsconfig_extends_root_base_inner(
    config_path: &Path,
    config: &Value,
    root_base_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<bool> {
    let Some(extends) = config.get("extends").and_then(Value::as_str) else {
        return Ok(false);
    };

    let parent_path = resolve_tsconfig_extends_path(config_path, extends);
    let normalized_parent_path = normalize_existing_path(&parent_path);
    let normalized_root_base = normalize_existing_path(root_base_path);

    if normalized_parent_path == normalized_root_base {
        return Ok(true);
    }

    if !visited.insert(normalized_parent_path.clone()) {
        return Ok(false);
    }

    let Some(text) = read_text_if_exists(&normalized_parent_path)? else {
        return Ok(false);
    };
    let Ok(parent_config) = parse_jsonc::<Value>(&text, &normalized_parent_path.to_string_lossy())
    else {
        return Ok(false);
    };

    tsconfig_extends_root_base_inner(
        &normalized_parent_path,
        &parent_config,
        &normalized_root_base,
        visited,
    )
}

fn resolve_tsconfig_extends_path(config_path: &Path, extends: &str) -> PathBuf {
    let base_dir = config_path.parent().unwrap_or(config_path);
    let candidate = Path::new(extends);
    let resolved = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };

    if resolved.is_file() {
        return resolved;
    }

    let json_candidate = resolved.with_extension("json");
    if json_candidate.is_file() {
        return json_candidate;
    }

    let directory_candidate = resolved.join("tsconfig.json");
    if directory_candidate.is_file() {
        return directory_candidate;
    }

    resolved
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
