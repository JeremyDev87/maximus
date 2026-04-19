use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use maximus_core::{
    get_files, make_finding, parse_jsonc, path_exists, read_text_if_exists, FileKind, Finding,
    FindingInput, ProjectSnapshot, Severity,
};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;

const RUNTIME_EXTENSIONS: &[&str] = &[
    ".cjs", ".cts", ".js", ".json", ".jsx", ".mjs", ".mts", ".node", ".ts", ".tsx",
];
const TYPES_EXTENSIONS: &[&str] = &[".d.cts", ".d.mts", ".d.ts"];
const MAX_NESTED_PACKAGE_DEPTH: usize = 8;
pub fn run_package_entrypoints_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for file in get_files(project, FileKind::Package) {
        let Some(text) = read_text_if_exists(&file.path)? else {
            continue;
        };

        let package_json = match parse_jsonc::<Value>(&text, &file.path.to_string_lossy()) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let package_dir = file.path.parent().unwrap_or(project.root_dir.as_path());
        collect_findings(
            &mut findings,
            &file.path,
            package_dir,
            &package_json,
            &["main", "module", "types", "bin", "exports", "imports"],
        )?;
    }

    findings = unique_findings_by_id(findings);

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn collect_findings(
    findings: &mut Vec<Finding>,
    package_path: &Path,
    package_dir: &Path,
    package_json: &Value,
    entrypoint_fields: &[&str],
) -> io::Result<()> {
    for field_name in entrypoint_fields {
        let Some(value) = package_json.get(field_name) else {
            continue;
        };

        scan_entrypoint_value(
            findings,
            package_path,
            package_dir,
            vec![field_name.to_string()],
            value,
        )?;
    }

    Ok(())
}

fn scan_entrypoint_value(
    findings: &mut Vec<Finding>,
    package_path: &Path,
    package_dir: &Path,
    field_path: Vec<String>,
    value: &Value,
) -> io::Result<()> {
    let outcome = inspect_entrypoint_value(package_path, package_dir, field_path, value)?;
    findings.extend(outcome.findings);
    Ok(())
}

#[derive(Default)]
struct InspectOutcome {
    findings: Vec<Finding>,
    has_valid_target: bool,
}

fn inspect_entrypoint_value(
    package_path: &Path,
    package_dir: &Path,
    field_path: Vec<String>,
    value: &Value,
) -> io::Result<InspectOutcome> {
    match value {
        Value::String(target) => {
            if let Some(detail) = invalid_local_target_detail(&field_path, target) {
                return Ok(InspectOutcome {
                    findings: vec![make_finding(FindingInput {
                        id: format!(
                            "package-entrypoints:{}:{}:{}",
                            package_path.to_string_lossy(),
                            field_path.join("/"),
                            target
                        ),
                        title: "Package entrypoint target is invalid".to_string(),
                        category: Some("package-entrypoints".to_string()),
                        detail: Some(detail),
                        file: Some(package_path.to_path_buf()),
                        fix_ids: Vec::new(),
                        fixable: false,
                        hint: Some(
                            "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments."
                                .to_string(),
                        ),
                        severity: Some(severity_for_field_path(&field_path)),
                    })],
                    has_valid_target: false,
                });
            }

            if field_path.first().is_some_and(|field| field == "imports")
                && is_valid_external_import_target(target)
            {
                return Ok(InspectOutcome {
                    findings: Vec::new(),
                    has_valid_target: true,
                });
            }

            if !is_relative_target(package_dir, &field_path, target)? {
                return Ok(InspectOutcome::default());
            }

            if target.contains('*') {
                let wildcard_outcome =
                    inspect_wildcard_target(package_path, package_dir, &field_path, target)?;
                if wildcard_outcome.has_valid_target || !wildcard_outcome.findings.is_empty() {
                    return Ok(wildcard_outcome);
                }

                return Ok(InspectOutcome {
                    findings: vec![make_finding(FindingInput {
                        id: format!(
                            "package-entrypoints:{}:{}:{}",
                            package_path.to_string_lossy(),
                            field_path.join("/"),
                            target
                        ),
                        title: "Package entrypoint target does not exist".to_string(),
                        category: Some("package-entrypoints".to_string()),
                        detail: Some(format!(
                            "package.json {} points to {}, but the resolved path was not found.",
                            field_path.join("/"),
                            target
                        )),
                        file: Some(package_path.to_path_buf()),
                        fix_ids: Vec::new(),
                        fixable: false,
                        hint: Some(
                            "Update the relative path or remove the stale entrypoint before publishing."
                                .to_string(),
                        ),
                        severity: Some(severity_for_field_path(&field_path)),
                    })],
                    has_valid_target: false,
                });
            }

            if let Some(finding) =
                incompatible_exact_file_finding(package_path, package_dir, &field_path, target)
            {
                return Ok(InspectOutcome {
                    findings: vec![finding],
                    has_valid_target: false,
                });
            }

            let nested_directory_outcome =
                nested_directory_outcome(package_dir, &field_path, target, package_path)?;

            if relative_target_exists(package_dir, &field_path, target, package_path)? {
                return Ok(InspectOutcome {
                    findings: nested_directory_outcome.findings,
                    has_valid_target: true,
                });
            }

            if !nested_directory_outcome.findings.is_empty() {
                let mut findings = nested_directory_outcome.findings;
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "package-entrypoints:{}:{}:{}",
                        package_path.to_string_lossy(),
                        field_path.join("/"),
                        target
                    ),
                    title: "Package entrypoint target does not exist".to_string(),
                    category: Some("package-entrypoints".to_string()),
                    detail: Some(format!(
                        "package.json {} points to {}, but the resolved path was not found.",
                        field_path.join("/"),
                        target
                    )),
                    file: Some(package_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Update the relative path or remove the stale entrypoint before publishing."
                            .to_string(),
                    ),
                    severity: Some(severity_for_field_path(&field_path)),
                }));
                return Ok(InspectOutcome {
                    findings,
                    has_valid_target: false,
                });
            }

            Ok(InspectOutcome {
                findings: vec![make_finding(FindingInput {
                    id: format!(
                        "package-entrypoints:{}:{}:{}",
                        package_path.to_string_lossy(),
                        field_path.join("/"),
                        target
                    ),
                    title: "Package entrypoint target does not exist".to_string(),
                    category: Some("package-entrypoints".to_string()),
                    detail: Some(format!(
                        "package.json {} points to {}, but the resolved path was not found.",
                        field_path.join("/"),
                        target
                    )),
                    file: Some(package_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Update the relative path or remove the stale entrypoint before publishing."
                            .to_string(),
                    ),
                    severity: Some(severity_for_field_path(&field_path)),
                })],
                has_valid_target: false,
            })
        }
        Value::Array(values) => {
            if !allows_array_target(&field_path) {
                return Ok(InspectOutcome {
                    findings: vec![invalid_entrypoint_value_finding(
                        package_path,
                        &field_path,
                        value,
                    )],
                    has_valid_target: false,
                });
            }

            let mut findings = Vec::new();

            for (index, branch) in values.iter().enumerate() {
                let mut next_path = field_path.clone();
                next_path.push(format!("[{index}]"));
                let branch_outcome =
                    inspect_entrypoint_value(package_path, package_dir, next_path, branch)?;

                if branch_outcome.has_valid_target {
                    findings.extend(retained_fallback_findings(branch_outcome.findings));
                    return Ok(InspectOutcome {
                        findings: unique_findings_by_id(retained_fallback_findings(findings)),
                        has_valid_target: true,
                    });
                }

                findings.extend(branch_outcome.findings);
            }

            if findings.len() > 1 {
                findings = vec![select_best_finding(findings)];
            }

            Ok(InspectOutcome {
                findings,
                has_valid_target: false,
            })
        }
        Value::Object(entries) => {
            if !allows_object_target(&field_path) {
                return Ok(InspectOutcome {
                    findings: vec![invalid_entrypoint_value_finding(
                        package_path,
                        &field_path,
                        value,
                    )],
                    has_valid_target: false,
                });
            }

            let mut findings = invalid_object_key_findings(package_path, &field_path, entries);
            let mut has_valid_target = false;

            for (key, branch) in entries {
                let mut next_path = field_path.clone();
                next_path.push(key.clone());
                let branch_outcome =
                    inspect_entrypoint_value(package_path, package_dir, next_path, branch)?;
                has_valid_target |= branch_outcome.has_valid_target;
                findings.extend(branch_outcome.findings);
            }

            if should_compress_object_branches(&field_path, entries) && findings.len() > 1 {
                findings = vec![select_best_finding(findings)];
            }

            Ok(InspectOutcome {
                findings,
                has_valid_target,
            })
        }
        Value::Null if allows_null_target(&field_path) => Ok(InspectOutcome::default()),
        _ => Ok(InspectOutcome {
            findings: vec![invalid_entrypoint_value_finding(
                package_path,
                &field_path,
                value,
            )],
            has_valid_target: false,
        }),
    }
}

fn invalid_entrypoint_value_finding(
    package_path: &Path,
    field_path: &[String],
    value: &Value,
) -> Finding {
    let value_kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };

    make_finding(FindingInput {
        id: format!(
            "package-entrypoints:{}:{}:invalid-type:{}",
            package_path.to_string_lossy(),
            field_path.join("/"),
            value_kind
        ),
        title: "Package entrypoint target is invalid".to_string(),
        category: Some("package-entrypoints".to_string()),
        detail: Some(format!(
            "package.json {} must be a string target or nested fallback branches, but found {}.",
            field_path.join("/"),
            value_kind
        )),
        file: Some(package_path.to_path_buf()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some(
            "Use a string target or nested fallback branches composed of strings, arrays, and objects."
                .to_string(),
        ),
        severity: Some(severity_for_field_path(field_path)),
    })
}

fn relative_target_exists(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    package_path: &Path,
) -> io::Result<bool> {
    relative_target_exists_at_depth(base_dir, field_path, target, package_path, 0)
}

fn relative_target_exists_at_depth(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    package_path: &Path,
    depth: usize,
) -> io::Result<bool> {
    if target.contains('*') {
        return wildcard_target_exists(base_dir, field_path, target);
    }

    static_target_exists_at_depth(base_dir, field_path, target, package_path, depth)
}

fn static_target_exists(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    package_path: &Path,
) -> io::Result<bool> {
    static_target_exists_at_depth(base_dir, field_path, target, package_path, 0)
}

fn static_target_exists_at_depth(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    package_path: &Path,
    depth: usize,
) -> io::Result<bool> {
    let stem = target.split('*').next().unwrap_or(target);
    let Some(resolved) = resolve_path(base_dir, stem) else {
        return Ok(false);
    };

    if resolved.is_file() {
        return Ok(true);
    }

    if has_supported_explicit_extension(field_path, target) {
        return Ok(false);
    }

    for candidate in build_extension_candidates(field_path, &resolved) {
        if candidate.is_file() {
            return Ok(true);
        }
    }

    if resolved.is_dir() && allows_directory_target_resolution(field_path) {
        return directory_target_exists_at_depth(&resolved, field_path, package_path, depth);
    }

    if allows_index_directory_fallback(field_path) {
        let directory_candidate = resolved.join("index");
        for candidate in build_extension_candidates(field_path, &directory_candidate) {
            if candidate.is_file() {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn wildcard_target_exists(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
) -> io::Result<bool> {
    let mut segments = target.split('*');
    let prefix = segments.next().unwrap_or_default();
    let _rest = segments.collect::<Vec<_>>();
    let search_prefix = wildcard_search_prefix(prefix);
    let Some(search_root) = resolve_path(base_dir, search_prefix) else {
        return Ok(false);
    };

    if !path_exists(&search_root) {
        return Ok(false);
    }

    let patterns = build_wildcard_patterns(base_dir, field_path, target);
    Ok(!collect_matching_paths(&search_root, &patterns)?.is_empty())
}

fn collect_matching_paths(candidate_path: &Path, patterns: &[String]) -> io::Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    collect_matching_paths_into(candidate_path, patterns, &mut matches)?;
    Ok(matches)
}

fn collect_matching_paths_into(
    candidate_path: &Path,
    patterns: &[String],
    matches: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if candidate_path.is_file() && matches_path(candidate_path, patterns) {
        matches.push(candidate_path.to_path_buf());
        return Ok(());
    }

    let entries = match fs::read_dir(candidate_path) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_file() && matches_path(&entry_path, patterns) {
            matches.push(entry_path);
            continue;
        }

        if file_type.is_dir() {
            collect_matching_paths_into(&entry_path, patterns, matches)?;
        }
    }

    Ok(())
}

fn matches_path(candidate_path: &Path, patterns: &[String]) -> bool {
    let normalized_path = normalize_path_for_match(candidate_path);
    patterns
        .iter()
        .any(|pattern| wildcard_pattern_matches(&normalized_path, pattern))
}

fn build_wildcard_patterns(base_dir: &Path, field_path: &[String], target: &str) -> Vec<String> {
    let Some(resolved) = resolve_path(base_dir, target) else {
        return Vec::new();
    };
    let mut patterns = vec![normalize_path_for_match(&resolved)];

    if !has_supported_explicit_extension(field_path, target) {
        for candidate in build_extension_candidates(field_path, &resolved) {
            patterns.push(normalize_path_for_match(&candidate));
        }

        if field_path
            .first()
            .is_some_and(|field| is_main_like_field(field))
        {
            let directory_candidate = resolved.join("index");
            for candidate in build_extension_candidates(field_path, &directory_candidate) {
                patterns.push(normalize_path_for_match(&candidate));
            }
        }
    }

    patterns
}

fn has_supported_explicit_extension(field_path: &[String], target: &str) -> bool {
    let tail = target.rsplit('*').next().unwrap_or(target);
    let Some(file_name) = Path::new(tail).file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    extensions_for_field_path(field_path)
        .iter()
        .any(|extension| file_name.ends_with(extension))
}

fn wildcard_search_prefix(prefix: &str) -> &str {
    if prefix.is_empty() {
        return ".";
    }

    if prefix.ends_with('/') || prefix.ends_with('\\') {
        return prefix;
    }

    match prefix.rfind(['/', '\\']) {
        Some(0) => &prefix[..1],
        Some(index) => &prefix[..index],
        None => ".",
    }
}

fn wildcard_pattern_matches(candidate: &str, pattern: &str) -> bool {
    let candidate_chars = candidate.chars().collect::<Vec<_>>();
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; pattern_chars.len() + 1]; candidate_chars.len() + 1];

    fn matches(
        candidate: &[char],
        pattern: &[char],
        candidate_index: usize,
        pattern_index: usize,
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(result) = memo[candidate_index][pattern_index] {
            return result;
        }

        let result = if pattern_index == pattern.len() {
            candidate_index == candidate.len()
        } else if pattern[pattern_index] == '*' {
            let mut offset = candidate_index + 1;
            let mut matched = false;
            while offset <= candidate.len() {
                if matches(candidate, pattern, offset, pattern_index + 1, memo) {
                    matched = true;
                    break;
                }
                offset += 1;
            }
            matched
        } else if candidate_index < candidate.len()
            && candidate[candidate_index] == pattern[pattern_index]
        {
            matches(
                candidate,
                pattern,
                candidate_index + 1,
                pattern_index + 1,
                memo,
            )
        } else {
            false
        };

        memo[candidate_index][pattern_index] = Some(result);
        result
    }

    matches(&candidate_chars, &pattern_chars, 0, 0, &mut memo)
}

fn normalize_path_for_match(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_relative_target(base_dir: &Path, field_path: &[String], target: &str) -> io::Result<bool> {
    let Some(root_field) = field_path.first() else {
        return Ok(false);
    };

    match root_field.as_str() {
        "exports" => Ok(is_exports_local_target(target)),
        "imports" => Ok(is_imports_local_target(target)),
        _ => is_main_like_local_target(base_dir, root_field, target),
    }
}

fn resolve_path(base_dir: &Path, target: &str) -> Option<PathBuf> {
    let base_dir = normalize_path(base_dir);
    let normalized_target = target.replace('\\', "/");
    let resolved = normalize_path(&base_dir.join(normalized_target));

    if resolved.starts_with(&base_dir) {
        Some(resolved)
    } else {
        None
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|value| matches!(value, Component::Normal(_)))
                {
                    normalized.pop();
                } else if normalized.as_os_str().is_empty() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn build_extension_candidates(field_path: &[String], path: &Path) -> Vec<PathBuf> {
    let path_string = path.to_string_lossy();
    extensions_for_field_path(field_path)
        .iter()
        .map(|extension| PathBuf::from(format!("{path_string}{extension}")))
        .collect()
}

fn directory_target_exists_at_depth(
    path: &Path,
    field_path: &[String],
    package_path: &Path,
    depth: usize,
) -> io::Result<bool> {
    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(false);
    }

    let package_manifest_path = path.join("package.json");
    if package_manifest_path.is_file() && package_manifest_path != package_path {
        if let Some(text) = read_text_if_exists(&package_manifest_path)? {
            let package_json =
                match parse_jsonc::<Value>(&text, &package_manifest_path.to_string_lossy()) {
                    Ok(package_json) => package_json,
                    Err(_) => return Ok(false),
                };
            if let Some(root_field) = field_path.first() {
                if let Some(value) = package_json.get(root_field) {
                    let nested_field_path = vec![root_field.clone()];
                    let nested_outcome = inspect_nested_directory_value_at_depth(
                        &package_manifest_path,
                        path,
                        nested_field_path,
                        value,
                        depth + 1,
                    )?;
                    if nested_outcome.has_valid_target {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(build_extension_candidates(field_path, &path.join("index"))
        .iter()
        .any(|candidate| candidate.is_file()))
}

fn is_main_like_field(field: &str) -> bool {
    matches!(field, "main" | "module" | "types" | "bin")
}

fn is_main_like_local_target(base_dir: &Path, root_field: &str, target: &str) -> io::Result<bool> {
    if target.is_empty() || has_url_like_scheme(target) || target.starts_with('#') {
        return Ok(false);
    }

    if target.starts_with("./") || target.starts_with("../") {
        return Ok(true);
    }

    if target.starts_with('/') || target.starts_with('\\') {
        return Ok(false);
    }

    let root_field_path = [root_field.to_string()];
    let package_path = base_dir.join("package.json");
    if static_target_exists(base_dir, &root_field_path, target, &package_path)? {
        return Ok(true);
    }

    let normalized = target.replace('\\', "/");
    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        [] => Ok(false),
        [..] => Ok(true),
    }
}

fn is_exports_local_target(target: &str) -> bool {
    if target.is_empty() || has_url_like_scheme(target) || target.starts_with('#') {
        return false;
    }

    let normalized = target.replace('\\', "/");
    normalized.starts_with("./")
}

fn is_imports_local_target(target: &str) -> bool {
    if target.is_empty() || has_url_like_scheme(target) || target.starts_with('#') {
        return false;
    }

    let normalized = target.replace('\\', "/");
    normalized.starts_with("./") || normalized.starts_with("../")
}

fn has_url_like_scheme(target: &str) -> bool {
    let Some(index) = target.find(':') else {
        return false;
    };

    if index == 0 {
        return false;
    }

    let scheme = &target[..index];
    if !scheme
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || matches!(value, '+' | '-' | '.'))
    {
        return false;
    }

    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "blob"
            | "data"
            | "file"
            | "ftp"
            | "ftps"
            | "http"
            | "https"
            | "javascript"
            | "mailto"
            | "node"
            | "ws"
            | "wss"
    )
}

fn should_compress_object_branches(
    field_path: &[String],
    entries: &serde_json::Map<String, Value>,
) -> bool {
    if field_path.len() > 1 {
        return true;
    }

    let Some(root_field) = field_path.first() else {
        return false;
    };

    match root_field.as_str() {
        "exports" => entries.keys().all(|key| !key.starts_with('.')),
        "imports" => entries.keys().all(|key| !key.starts_with('#')),
        _ => false,
    }
}

fn severity_for_field_path(field_path: &[String]) -> Severity {
    if uses_types_extensions(field_path) {
        Severity::Warn
    } else {
        Severity::Error
    }
}

fn select_best_finding(mut findings: Vec<Finding>) -> Finding {
    findings.sort_by_key(|finding| match finding.severity {
        Severity::Error => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    });
    findings.remove(0)
}

fn extensions_for_field_path(field_path: &[String]) -> &'static [&'static str] {
    if uses_types_extensions(field_path) {
        TYPES_EXTENSIONS
    } else {
        RUNTIME_EXTENSIONS
    }
}

fn uses_types_extensions(field_path: &[String]) -> bool {
    match field_path.first().map(String::as_str) {
        Some("types") => true,
        Some("exports" | "imports") => field_path.iter().skip(1).any(|segment| segment == "types"),
        _ => false,
    }
}

fn invalid_local_target_detail(field_path: &[String], target: &str) -> Option<String> {
    let root_field = field_path.first()?;
    let reason = match root_field.as_str() {
        "exports" => invalid_exports_target_reason(target)?,
        "imports" => invalid_imports_target_reason(target)?,
        "main" | "module" | "types" | "bin" => invalid_main_like_target_reason(target)?,
        _ => return None,
    };
    Some(format!(
        "package.json {} points to {}, but {}.",
        field_path.join("/"),
        target,
        reason
    ))
}

fn invalid_object_key_findings(
    package_path: &Path,
    field_path: &[String],
    entries: &serde_json::Map<String, Value>,
) -> Vec<Finding> {
    let Some(root_field) = field_path.first() else {
        return Vec::new();
    };

    match root_field.as_str() {
        "imports" if field_path.len() == 1 => entries
            .keys()
            .filter_map(|key| invalid_imports_key_reason(key).map(|reason| (key, reason)))
            .map(|(key, reason)| {
                make_finding(FindingInput {
                    id: format!(
                        "package-entrypoints:{}:{}/{}:key",
                        package_path.to_string_lossy(),
                        field_path.join("/"),
                        key
                    ),
                    title: "Package entrypoint key is invalid".to_string(),
                    category: Some("package-entrypoints".to_string()),
                    detail: Some(format!(
                        "package.json imports/{key} uses an invalid key. {reason}."
                    )),
                    file: Some(package_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Rename the imports entry to a # alias before publishing.".to_string(),
                    ),
                    severity: Some(Severity::Error),
                })
            })
            .collect(),
        "exports" => {
            let mut findings = Vec::new();

            if field_path.len() == 1 && mixes_exports_subpaths_and_conditions(entries) {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "package-entrypoints:{}:{}:mixed-keys",
                        package_path.to_string_lossy(),
                        field_path.join("/")
                    ),
                    title: "Package exports object is invalid".to_string(),
                    category: Some("package-entrypoints".to_string()),
                    detail: Some(
                        "package.json exports mixes subpath keys with condition keys at the same object level."
                            .to_string(),
                    ),
                    file: Some(package_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Use either package subpath keys such as . and ./feature, or conditional keys such as import and default at one level."
                            .to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
            }

            findings.extend(
                entries
                    .keys()
                    .filter_map(|key| invalid_exports_key_reason(key).map(|reason| (key, reason)))
                    .map(|(key, reason)| {
                        make_finding(FindingInput {
                            id: format!(
                                "package-entrypoints:{}:{}/{}:key",
                                package_path.to_string_lossy(),
                                field_path.join("/"),
                                key
                            ),
                            title: "Package entrypoint key is invalid".to_string(),
                            category: Some("package-entrypoints".to_string()),
                            detail: Some(format!(
                                "package.json {}/{} uses an invalid key. {reason}.",
                                field_path.join("/"),
                                key
                            )),
                            file: Some(package_path.to_path_buf()),
                            fix_ids: Vec::new(),
                            fixable: false,
                            hint: Some(exports_key_hint(key).to_string()),
                            severity: Some(Severity::Error),
                        })
                    }),
            );

            findings
        }
        _ => Vec::new(),
    }
}

fn mixes_exports_subpaths_and_conditions(entries: &serde_json::Map<String, Value>) -> bool {
    let has_subpath_key = entries.keys().any(|key| key.starts_with('.'));
    let has_condition_key = entries.keys().any(|key| !key.starts_with('.'));
    has_subpath_key && has_condition_key
}

fn invalid_exports_target_reason(target: &str) -> Option<&'static str> {
    let normalized = target.replace('\\', "/");

    if !normalized.starts_with("./") {
        return Some("exports targets must stay within the package and start with ./");
    }

    let remainder = normalized.trim_start_matches("./");
    if remainder.is_empty() {
        return Some("exports targets must point to a file or directory under ./");
    }

    if has_invalid_package_path_segment(remainder) {
        return Some("exports targets cannot contain empty, ., .., or node_modules path segments");
    }

    None
}

fn invalid_main_like_target_reason(target: &str) -> Option<&'static str> {
    if target.is_empty() {
        return Some("main/module/types/bin targets must not be empty");
    }

    if target.contains('*') {
        return Some("main/module/types/bin targets must not use wildcard patterns");
    }

    if has_url_like_scheme(target) {
        return Some(
            "main/module/types/bin targets must be package-local paths and cannot use URL-like schemes",
        );
    }

    if target.starts_with('#') {
        return Some(
            "main/module/types/bin targets must be package-local paths and cannot use # references",
        );
    }

    if is_absolute_like_target(target) {
        return Some(
            "main/module/types/bin targets must be package-local paths and cannot use absolute paths",
        );
    }

    let normalized = target.replace('\\', "/");
    let remainder = normalized.strip_prefix("./").unwrap_or(&normalized);
    if remainder.is_empty() {
        return Some("main/module/types/bin targets must point to a file or directory under ./");
    }
    if has_invalid_package_path_segment(remainder) {
        return Some(
            "main/module/types/bin targets must stay within the package and cannot contain empty, ., .., or node_modules path segments",
        );
    }

    None
}

fn is_absolute_like_target(target: &str) -> bool {
    if target.starts_with('/') || target.starts_with('\\') {
        return true;
    }

    let bytes = target.as_bytes();
    bytes.len() >= 3
        && bytes[1] == b':'
        && bytes[0].is_ascii_alphabetic()
        && matches!(bytes[2], b'/' | b'\\')
}

fn invalid_imports_target_reason(target: &str) -> Option<&'static str> {
    if target.is_empty() {
        return Some("imports targets must not be empty");
    }

    if is_absolute_like_target(target) {
        return Some("imports local targets must stay within the package and start with ./");
    }

    let normalized = target.replace('\\', "/");

    if normalized.starts_with("../") || normalized.starts_with('/') {
        return Some("imports local targets must stay within the package and start with ./");
    }

    if !normalized.starts_with("./") {
        if is_valid_external_import_target(target) {
            return None;
        }
        return Some(
            "imports targets must be package-local paths under ./ or valid external package specifiers",
        );
    }

    let remainder = normalized.trim_start_matches("./");
    if remainder.is_empty() {
        return Some("imports local targets must point to a file or directory under ./");
    }

    if has_invalid_package_path_segment(remainder) {
        return Some(
            "imports local targets cannot contain empty, ., .., or node_modules path segments",
        );
    }

    None
}

fn is_valid_external_import_target(target: &str) -> bool {
    if target.is_empty()
        || target.starts_with('.')
        || target.starts_with('/')
        || target.starts_with('\\')
        || target.starts_with('#')
        || target.contains('\\')
    {
        return false;
    }

    if target.contains(':') {
        return false;
    }

    let segments = target.split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return false;
    }

    if let Some(scope) = segments.first().filter(|segment| segment.starts_with('@')) {
        return scope.len() > 1
            && segments.len() >= 2
            && !matches!(segments[1], "." | "..")
            && segments[2..]
                .iter()
                .all(|segment| !matches!(*segment, "." | ".."));
    }

    !matches!(segments[0], "." | "..")
        && segments[1..]
            .iter()
            .all(|segment| !matches!(*segment, "." | ".."))
}

fn has_invalid_package_path_segment(remainder: &str) -> bool {
    let decoded = percent_decode_path_like_node(remainder).replace('\\', "/");
    decoded
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".." | "node_modules"))
}

fn percent_decode_path_like_node(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = bytes[index + 1];
            let lo = bytes[index + 2];
            if let (Some(hi), Some(lo)) = (hex_value(hi), hex_value(lo)) {
                output.push((hi * 16 + lo) as char);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn allows_null_target(field_path: &[String]) -> bool {
    field_path
        .first()
        .is_some_and(|field| matches!(field.as_str(), "exports" | "imports"))
}

fn invalid_imports_key_reason(key: &str) -> Option<&'static str> {
    if key == "#" {
        Some("imports keys must include a name after the # prefix")
    } else if let Some(remainder) = key.strip_prefix("#/") {
        if remainder.is_empty() {
            Some("imports keys must include a name after the #/ prefix")
        } else if has_invalid_package_key_segment(remainder) {
            Some("imports keys must not contain empty, ., .., or node_modules path segments")
        } else {
            None
        }
    } else if let Some(remainder) = key.strip_prefix('#') {
        if remainder.is_empty() {
            Some("imports keys must include a name after the # prefix")
        } else if has_invalid_package_key_segment(remainder) {
            Some("imports keys must not contain empty, ., .., or node_modules path segments")
        } else {
            None
        }
    } else {
        Some("imports keys must start with #")
    }
}

fn invalid_exports_key_reason(key: &str) -> Option<&'static str> {
    if key.starts_with('#') {
        Some("exports keys must not start with #")
    } else if key == "." {
        None
    } else if let Some(remainder) = key.strip_prefix("./") {
        if remainder.is_empty() || has_invalid_package_key_segment(remainder) {
            Some(
                "exports subpath keys must not contain empty, ., .., or node_modules path segments",
            )
        } else {
            None
        }
    } else {
        None
    }
}

fn exports_key_hint(key: &str) -> &'static str {
    if key.starts_with('#') {
        "Rename the exports entry to . or ./subpath, or use a condition key without a # prefix."
    } else {
        "Rename the exports entry to . or a ./subpath that stays within the package."
    }
}

fn has_invalid_package_key_segment(remainder: &str) -> bool {
    let decoded = percent_decode_path_like_node(remainder).replace('\\', "/");
    decoded
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".." | "node_modules"))
}

fn unique_findings_by_id(findings: Vec<Finding>) -> Vec<Finding> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();

    for finding in findings {
        if seen.insert(finding.id.clone()) {
            unique.push(finding);
        }
    }

    unique
}

fn incompatible_exact_file_finding(
    package_path: &Path,
    base_dir: &Path,
    field_path: &[String],
    target: &str,
) -> Option<Finding> {
    if target.contains('*') {
        return None;
    }

    let resolved = resolve_path(base_dir, target)?;
    if !resolved.is_file() || exact_file_matches_field_path(field_path, &resolved) {
        return None;
    }

    let detail = if uses_types_extensions(field_path) {
        format!(
            "package.json {} points to {}, but types targets must point to declaration files such as .d.ts, .d.mts, or .d.cts.",
            field_path.join("/"),
            target
        )
    } else {
        format!(
            "package.json {} points to {}, but runtime entrypoints must not point to declaration-only files such as .d.ts.",
            field_path.join("/"),
            target
        )
    };

    let hint = if uses_types_extensions(field_path) {
        "Point types to a generated declaration file before publishing.".to_string()
    } else {
        "Point main/module/bin to a runtime file instead of a declaration-only file.".to_string()
    };

    Some(make_finding(FindingInput {
        id: format!(
            "package-entrypoints:{}:{}:{}",
            package_path.to_string_lossy(),
            field_path.join("/"),
            target
        ),
        title: "Package entrypoint target uses an incompatible file type".to_string(),
        category: Some("package-entrypoints".to_string()),
        detail: Some(detail),
        file: Some(package_path.to_path_buf()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some(hint),
        severity: Some(severity_for_field_path(field_path)),
    }))
}

fn inspect_wildcard_target(
    package_path: &Path,
    base_dir: &Path,
    field_path: &[String],
    target: &str,
) -> io::Result<InspectOutcome> {
    inspect_wildcard_target_at_depth(package_path, base_dir, field_path, target, 0)
}

fn inspect_wildcard_target_at_depth(
    package_path: &Path,
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    depth: usize,
) -> io::Result<InspectOutcome> {
    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(InspectOutcome::default());
    }

    let mut segments = target.split('*');
    let prefix = segments.next().unwrap_or_default();
    let search_prefix = wildcard_search_prefix(prefix);
    let Some(search_root) = resolve_path(base_dir, search_prefix) else {
        return Ok(InspectOutcome::default());
    };

    if !path_exists(&search_root) {
        return Ok(InspectOutcome::default());
    }

    let patterns = build_wildcard_patterns(base_dir, field_path, target);
    let matches = collect_matching_paths(&search_root, &patterns)?;
    if matches.is_empty() {
        return Ok(InspectOutcome::default());
    }

    let mut findings = Vec::new();
    let mut has_valid_target = false;
    let current_package_path = normalize_path(package_path);

    for matched_path in matches {
        if matched_path
            .file_name()
            .is_some_and(|value| value == "package.json")
        {
            if normalize_path(&matched_path) == current_package_path {
                continue;
            }

            let package_outcome =
                inspect_package_manifest_outcome_at_depth(&matched_path, field_path, depth + 1)?;
            has_valid_target |= package_outcome.has_valid_target;
            findings.extend(package_outcome.findings);
            continue;
        }

        if exact_file_matches_field_path(field_path, &matched_path) {
            has_valid_target = true;
            continue;
        }

        findings.push(wildcard_incompatible_file_finding(
            package_path,
            base_dir,
            field_path,
            target,
            &matched_path,
        ));
    }

    Ok(InspectOutcome {
        findings: unique_findings_by_id(findings),
        has_valid_target,
    })
}

fn wildcard_incompatible_file_finding(
    package_path: &Path,
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    matched_path: &Path,
) -> Finding {
    let matched_display = display_relative_path(base_dir, matched_path);
    let detail = if uses_types_extensions(field_path) {
        format!(
            "package.json {} points to {}, but wildcard match {} is not a declaration file.",
            field_path.join("/"),
            target,
            matched_display
        )
    } else {
        format!(
            "package.json {} points to {}, but wildcard match {} is a declaration-only file.",
            field_path.join("/"),
            target,
            matched_display
        )
    };

    let hint = if uses_types_extensions(field_path) {
        "Point types to a generated declaration file before publishing.".to_string()
    } else {
        "Point runtime entrypoints to runtime files instead of declaration-only files.".to_string()
    };

    make_finding(FindingInput {
        id: format!(
            "package-entrypoints:{}:{}:{}:{}",
            package_path.to_string_lossy(),
            field_path.join("/"),
            target,
            matched_display
        ),
        title: "Package entrypoint target uses an incompatible file type".to_string(),
        category: Some("package-entrypoints".to_string()),
        detail: Some(detail),
        file: Some(package_path.to_path_buf()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some(hint),
        severity: Some(severity_for_field_path(field_path)),
    })
}

fn display_relative_path(base_dir: &Path, path: &Path) -> String {
    match path.strip_prefix(base_dir) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => format!("./{}", relative.to_string_lossy().replace('\\', "/")),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

fn exact_file_matches_field_path(field_path: &[String], path: &Path) -> bool {
    let path_string = path.to_string_lossy();
    let is_declaration_file = TYPES_EXTENSIONS
        .iter()
        .any(|extension| path_string.ends_with(extension));

    if uses_types_extensions(field_path) {
        is_declaration_file
    } else {
        !is_declaration_file
    }
}

fn nested_directory_outcome(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    current_package_path: &Path,
) -> io::Result<InspectOutcome> {
    nested_directory_outcome_at_depth(base_dir, field_path, target, current_package_path, 0)
}

fn nested_directory_outcome_at_depth(
    base_dir: &Path,
    field_path: &[String],
    target: &str,
    current_package_path: &Path,
    depth: usize,
) -> io::Result<InspectOutcome> {
    if target.contains('*') {
        return Ok(InspectOutcome::default());
    }

    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(InspectOutcome::default());
    }

    let Some(resolved) = resolve_path(base_dir, target) else {
        return Ok(InspectOutcome::default());
    };

    if !resolved.is_dir() {
        return Ok(InspectOutcome::default());
    }

    let package_manifest_path = resolved.join("package.json");
    if !package_manifest_path.is_file() || package_manifest_path == current_package_path {
        return Ok(InspectOutcome::default());
    }

    inspect_package_manifest_outcome_at_depth(&package_manifest_path, field_path, depth + 1)
}

fn inspect_package_manifest_outcome_at_depth(
    package_manifest_path: &Path,
    field_path: &[String],
    depth: usize,
) -> io::Result<InspectOutcome> {
    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(InspectOutcome::default());
    }

    let Some(text) = read_text_if_exists(package_manifest_path)? else {
        return Ok(InspectOutcome::default());
    };

    let package_json = match parse_jsonc::<Value>(&text, &package_manifest_path.to_string_lossy()) {
        Ok(package_json) => package_json,
        Err(_) => return Ok(InspectOutcome::default()),
    };

    let known_fields = ["main", "module", "types", "bin", "exports", "imports"];
    let current_root_field = field_path.first().map(String::as_str);
    let current_root_has_index_fallback = current_root_field.is_some()
        && has_index_fallback(
            package_manifest_path
                .parent()
                .unwrap_or_else(|| Path::new(".")),
            field_path,
        );
    let mut findings = Vec::new();
    let mut has_valid_target = false;

    for root_field in known_fields {
        let Some(value) = package_json.get(root_field) else {
            continue;
        };

        let nested_outcome = inspect_nested_directory_value_at_depth(
            package_manifest_path,
            package_manifest_path
                .parent()
                .unwrap_or_else(|| Path::new(".")),
            vec![root_field.to_string()],
            value,
            depth + 1,
        )?;

        if Some(root_field) == current_root_field {
            if nested_outcome.has_valid_target {
                has_valid_target = true;
                findings.extend(nested_outcome.findings);
            } else if current_root_has_index_fallback {
                has_valid_target = true;
                findings.extend(retained_index_fallback_findings(nested_outcome.findings));
            } else {
                findings.extend(nested_outcome.findings);
            }
        } else {
            findings.extend(nested_outcome.findings);
        }
    }

    Ok(InspectOutcome {
        findings: unique_findings_by_id(findings),
        has_valid_target,
    })
}

fn has_index_fallback(package_dir: &Path, field_path: &[String]) -> bool {
    if !allows_index_directory_fallback(field_path) {
        return false;
    }

    build_extension_candidates(field_path, &package_dir.join("index"))
        .iter()
        .any(|candidate| candidate.is_file())
}

fn allows_directory_target_resolution(field_path: &[String]) -> bool {
    field_path
        .first()
        .is_some_and(|field| !matches!(field.as_str(), "bin"))
}

fn allows_index_directory_fallback(field_path: &[String]) -> bool {
    field_path
        .first()
        .is_some_and(|field| matches!(field.as_str(), "main" | "module" | "types"))
}

fn inspect_nested_directory_value_at_depth(
    package_path: &Path,
    package_dir: &Path,
    field_path: Vec<String>,
    value: &Value,
    depth: usize,
) -> io::Result<InspectOutcome> {
    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(InspectOutcome::default());
    }

    match value {
        Value::String(target) => inspect_nested_directory_target_at_depth(
            package_path,
            package_dir,
            &field_path,
            target,
            depth,
        ),
        Value::Array(values) => {
            if !allows_array_target(&field_path) {
                return Ok(InspectOutcome {
                    findings: vec![invalid_entrypoint_value_finding(
                        package_path,
                        &field_path,
                        value,
                    )],
                    has_valid_target: false,
                });
            }

            let mut findings = Vec::new();

            for (index, branch) in values.iter().enumerate() {
                let mut next_path = field_path.clone();
                next_path.push(format!("[{index}]"));
                let branch_outcome = inspect_nested_directory_value_at_depth(
                    package_path,
                    package_dir,
                    next_path,
                    branch,
                    depth + 1,
                )?;
                if branch_outcome.has_valid_target {
                    findings.extend(retained_fallback_findings(branch_outcome.findings));
                    return Ok(InspectOutcome {
                        findings: unique_findings_by_id(retained_fallback_findings(findings)),
                        has_valid_target: true,
                    });
                }
                findings.extend(branch_outcome.findings);
            }

            Ok(InspectOutcome {
                findings,
                has_valid_target: false,
            })
        }
        Value::Object(entries) => {
            if !allows_object_target(&field_path) {
                return Ok(InspectOutcome {
                    findings: vec![invalid_entrypoint_value_finding(
                        package_path,
                        &field_path,
                        value,
                    )],
                    has_valid_target: false,
                });
            }

            let mut findings = invalid_object_key_findings(package_path, &field_path, entries);
            let mut has_valid_target = false;

            for (key, branch) in entries {
                let mut next_path = field_path.clone();
                next_path.push(key.clone());
                let branch_outcome = inspect_nested_directory_value_at_depth(
                    package_path,
                    package_dir,
                    next_path,
                    branch,
                    depth + 1,
                )?;
                has_valid_target |= branch_outcome.has_valid_target;
                findings.extend(branch_outcome.findings);
            }

            Ok(InspectOutcome {
                findings,
                has_valid_target,
            })
        }
        Value::Null if allows_null_target(&field_path) => Ok(InspectOutcome::default()),
        _ => Ok(InspectOutcome {
            findings: vec![invalid_entrypoint_value_finding(
                package_path,
                &field_path,
                value,
            )],
            has_valid_target: false,
        }),
    }
}

fn retained_fallback_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| !is_skippable_fallback_branch_finding(finding))
        .collect()
}

fn retained_index_fallback_findings(findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|finding| finding.title != "Package entrypoint target does not exist")
        .collect()
}

fn is_skippable_fallback_branch_finding(finding: &Finding) -> bool {
    match finding.title.as_str() {
        "Package entrypoint target does not exist" => true,
        "Package entrypoint target uses an incompatible file type" => true,
        "Package entrypoint target is invalid" => !finding.id.contains(":invalid-type:"),
        _ => false,
    }
}

fn allows_array_target(field_path: &[String]) -> bool {
    field_path
        .first()
        .is_some_and(|field| matches!(field.as_str(), "exports" | "imports"))
}

fn allows_object_target(field_path: &[String]) -> bool {
    match field_path.first().map(String::as_str) {
        Some("exports" | "imports") => true,
        Some("bin") => field_path.len() == 1,
        _ => false,
    }
}

fn inspect_nested_directory_target_at_depth(
    package_path: &Path,
    package_dir: &Path,
    field_path: &[String],
    target: &str,
    depth: usize,
) -> io::Result<InspectOutcome> {
    if depth >= MAX_NESTED_PACKAGE_DEPTH {
        return Ok(InspectOutcome::default());
    }

    if let Some(detail) = invalid_local_target_detail(field_path, target) {
        return Ok(InspectOutcome {
            findings: vec![make_finding(FindingInput {
                id: format!(
                    "package-entrypoints:{}:{}:{}",
                    package_path.to_string_lossy(),
                    field_path.join("/"),
                    target
                ),
                title: "Package entrypoint target is invalid".to_string(),
                category: Some("package-entrypoints".to_string()),
                detail: Some(detail),
                file: Some(package_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Use a package-local target that stays under ./ and avoids ., .., or node_modules segments."
                        .to_string(),
                ),
                severity: Some(severity_for_field_path(field_path)),
            })],
            has_valid_target: false,
        });
    }

    if field_path.first().is_some_and(|field| field == "imports")
        && is_valid_external_import_target(target)
    {
        return Ok(InspectOutcome {
            findings: Vec::new(),
            has_valid_target: true,
        });
    }

    if !is_relative_target(package_dir, field_path, target)? {
        return Ok(InspectOutcome::default());
    }

    if target.contains('*') {
        let wildcard_outcome = inspect_wildcard_target_at_depth(
            package_path,
            package_dir,
            field_path,
            target,
            depth + 1,
        )?;
        if wildcard_outcome.has_valid_target || !wildcard_outcome.findings.is_empty() {
            return Ok(wildcard_outcome);
        }

        return Ok(InspectOutcome {
            findings: vec![make_finding(FindingInput {
                id: format!(
                    "package-entrypoints:{}:{}:{}",
                    package_path.to_string_lossy(),
                    field_path.join("/"),
                    target
                ),
                title: "Package entrypoint target does not exist".to_string(),
                category: Some("package-entrypoints".to_string()),
                detail: Some(format!(
                    "package.json {} points to {}, but the resolved path was not found.",
                    field_path.join("/"),
                    target
                )),
                file: Some(package_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Update the relative path or remove the stale entrypoint before publishing."
                        .to_string(),
                ),
                severity: Some(severity_for_field_path(field_path)),
            })],
            has_valid_target: false,
        });
    }

    if let Some(finding) =
        incompatible_exact_file_finding(package_path, package_dir, field_path, target)
    {
        return Ok(InspectOutcome {
            findings: vec![finding],
            has_valid_target: false,
        });
    }

    let nested_directory_outcome = nested_directory_outcome_at_depth(
        package_dir,
        field_path,
        target,
        package_path,
        depth + 1,
    )?;

    if relative_target_exists_at_depth(package_dir, field_path, target, package_path, depth + 1)? {
        return Ok(InspectOutcome {
            findings: nested_directory_outcome.findings,
            has_valid_target: true,
        });
    }

    if !nested_directory_outcome.findings.is_empty() {
        let mut findings = nested_directory_outcome.findings;
        findings.push(make_finding(FindingInput {
            id: format!(
                "package-entrypoints:{}:{}:{}",
                package_path.to_string_lossy(),
                field_path.join("/"),
                target
            ),
            title: "Package entrypoint target does not exist".to_string(),
            category: Some("package-entrypoints".to_string()),
            detail: Some(format!(
                "package.json {} points to {}, but the resolved path was not found.",
                field_path.join("/"),
                target
            )),
            file: Some(package_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Update the relative path or remove the stale entrypoint before publishing."
                    .to_string(),
            ),
            severity: Some(severity_for_field_path(field_path)),
        }));
        return Ok(InspectOutcome {
            findings,
            has_valid_target: false,
        });
    }

    Ok(InspectOutcome {
        findings: vec![make_finding(FindingInput {
            id: format!(
                "package-entrypoints:{}:{}:{}",
                package_path.to_string_lossy(),
                field_path.join("/"),
                target
            ),
            title: "Package entrypoint target does not exist".to_string(),
            category: Some("package-entrypoints".to_string()),
            detail: Some(format!(
                "package.json {} points to {}, but the resolved path was not found.",
                field_path.join("/"),
                target
            )),
            file: Some(package_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Update the relative path or remove the stale entrypoint before publishing."
                    .to_string(),
            ),
            severity: Some(severity_for_field_path(field_path)),
        })],
        has_valid_target: false,
    })
}
