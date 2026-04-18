use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use maximus_core::{
    find_nearest_package_file, get_files, make_finding, parse_jsonc, path_exists,
    read_text_if_exists, FileKind, Finding, FindingInput, FixPlan, ProjectSnapshot, Severity,
};
use serde_json::{Map, Value};

const DEPRECATED_COMPILER_OPTIONS: &[(&str, &str)] = &[
    (
        "charset",
        "Remove it. TypeScript ignores this option in modern versions.",
    ),
    (
        "importsNotUsedAsValues",
        "Prefer verbatimModuleSyntax in modern TypeScript.",
    ),
    (
        "keyofStringsOnly",
        "Remove it. Modern TypeScript no longer needs this compatibility flag.",
    ),
    (
        "noStrictGenericChecks",
        "Remove it and rely on strict mode checks instead.",
    ),
    ("out", "Use outFile if you truly need single-file emit."),
    (
        "preserveValueImports",
        "Prefer verbatimModuleSyntax in modern TypeScript.",
    ),
    (
        "suppressExcessPropertyErrors",
        "Remove it. This hides useful structural typing errors.",
    ),
    (
        "suppressImplicitAnyIndexErrors",
        "Remove it. This suppresses important type safety signals.",
    ),
];

const CHECKABLE_EXTENSIONS: &[&str] = &[
    ".cjs", ".cts", ".js", ".json", ".jsx", ".mjs", ".mts", ".ts", ".tsx",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOutcome {
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
}

pub fn run_tsconfig_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for file in get_files(project, FileKind::Tsconfig) {
        let Some(text) = read_text_if_exists(&file.path)? else {
            continue;
        };

        let config = match parse_jsonc::<Value>(&text, &file.path.to_string_lossy()) {
            Ok(config) => config,
            Err(error) => {
                findings.push(make_finding(FindingInput {
                    id: format!("tsconfig-parse:{}", file.path.to_string_lossy()),
                    title: "Config file could not be parsed".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(error.to_string()),
                    file: Some(file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Fix invalid JSONC syntax before relying on this config.".to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
        };

        let compiler_options = config.get("compilerOptions").and_then(Value::as_object);

        collect_deprecated_option_findings(&mut findings, &file.path, compiler_options);

        let Some(paths_config) = compiler_options.and_then(|options| options.get("paths")) else {
            continue;
        };

        let Some(paths_config) = paths_config.as_object() else {
            findings.push(make_finding(FindingInput {
                id: format!("tsconfig-paths-shape:{}", file.path.to_string_lossy()),
                title: "compilerOptions.paths must be an object".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "TypeScript expects alias keys mapped to arrays of target strings.".to_string(),
                ),
                file: Some(file.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Rewrite paths to the standard { alias: [targets] } shape.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        let base_dir = compiler_options
            .and_then(|options| options.get("baseUrl"))
            .and_then(Value::as_str)
            .map(|base_url| resolve_path(&file.dir, base_url))
            .unwrap_or_else(|| file.dir.clone());

        collect_paths_findings(&mut findings, &file.path, &base_dir, paths_config)?;

        if let Some(package_file) = find_nearest_package_file(project, &file.dir) {
            if let Some(package_text) = read_text_if_exists(&package_file.path)? {
                if let Ok(package_json) =
                    parse_jsonc::<Value>(&package_text, &package_file.path.to_string_lossy())
                {
                    let imports = package_json
                        .get("imports")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    compare_imports_and_paths(
                        &mut findings,
                        &file.path,
                        package_file
                            .path
                            .parent()
                            .unwrap_or(project.root_dir.as_path()),
                        &base_dir,
                        &imports,
                        paths_config,
                    );
                }
            }
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
    })
}

fn collect_deprecated_option_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    compiler_options: Option<&Map<String, Value>>,
) {
    let Some(compiler_options) = compiler_options else {
        return;
    };

    for (option, guidance) in DEPRECATED_COMPILER_OPTIONS {
        if !compiler_options.contains_key(*option) {
            continue;
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-deprecated:{}:{option}",
                file_path.to_string_lossy()
            ),
            title: format!("Deprecated compiler option \"{option}\""),
            category: Some("tsconfig".to_string()),
            detail: Some((*guidance).to_string()),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some("Remove legacy flags before they become upgrade blockers.".to_string()),
            severity: Some(Severity::Warn),
        }));
    }
}

fn collect_paths_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    base_dir: &Path,
    paths_config: &Map<String, Value>,
) -> io::Result<()> {
    for (alias, targets) in paths_config {
        let Some(targets) = targets.as_array() else {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-empty:{}:{alias}",
                    file_path.to_string_lossy()
                ),
                title: format!("Alias \"{alias}\" does not declare any targets"),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "Each path alias should map to at least one target string.".to_string(),
                ),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Add a valid target or remove the alias entry.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        if targets.is_empty() {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-empty:{}:{alias}",
                    file_path.to_string_lossy()
                ),
                title: format!("Alias \"{alias}\" does not declare any targets"),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "Each path alias should map to at least one target string.".to_string(),
                ),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Add a valid target or remove the alias entry.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        let alias_has_wildcard = alias.contains('*');
        for target in targets {
            let Some(target) = target.as_str() else {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-paths-type:{}:{alias}",
                        file_path.to_string_lossy()
                    ),
                    title: format!("Alias \"{alias}\" contains a non-string target"),
                    category: Some("tsconfig".to_string()),
                    detail: Some("TypeScript path targets must be strings.".to_string()),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some("Replace non-string entries with valid path strings.".to_string()),
                    severity: Some(Severity::Error),
                }));
                continue;
            };

            let target_has_wildcard = target.contains('*');
            if alias_has_wildcard != target_has_wildcard {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-paths-wildcard:{}:{alias}:{target}",
                        file_path.to_string_lossy()
                    ),
                    title: format!("Wildcard shape does not match for alias \"{alias}\""),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "{alias} maps to {target}, but only one side uses \"*\"."
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Keep wildcard placement aligned so imports resolve predictably."
                            .to_string(),
                    ),
                    severity: Some(Severity::Warn),
                }));
            }

            if !alias_target_exists(base_dir, target)? {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-paths-missing:{}:{alias}:{target}",
                        file_path.to_string_lossy()
                    ),
                    title: "Path alias target does not exist".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "{alias} points to {target}, but the resolved path was not found."
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Update or remove stale aliases before they break editor and build resolution."
                            .to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
            }
        }
    }

    Ok(())
}

fn compare_imports_and_paths(
    findings: &mut Vec<Finding>,
    tsconfig_path: &Path,
    package_dir: &Path,
    tsconfig_base_dir: &Path,
    imports: &Map<String, Value>,
    paths_config: &Map<String, Value>,
) {
    for (import_key, import_target) in imports {
        let Some(ts_targets) = paths_config.get(import_key).and_then(Value::as_array) else {
            continue;
        };
        let Some(first_ts_target) = ts_targets.first().and_then(Value::as_str) else {
            continue;
        };

        let normalized_import_targets = normalize_import_targets(package_dir, import_target);
        let Some(normalized_ts_target) =
            normalize_comparable_target(tsconfig_base_dir, first_ts_target)
        else {
            continue;
        };

        if normalized_import_targets.is_empty()
            || normalized_import_targets.contains(&normalized_ts_target)
        {
            continue;
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-import-conflict:{}:{import_key}",
                tsconfig_path.to_string_lossy()
            ),
            title: format!(
                "Alias \"{import_key}\" differs between tsconfig and package imports"
            ),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "tsconfig resolves to {first_ts_target}, while package.json imports resolves to {}.",
                stringify_import_target(import_target)
            )),
            file: Some(tsconfig_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Align both alias surfaces so runtime and editor resolution stay consistent."
                    .to_string(),
            ),
            severity: Some(Severity::Warn),
        }));
    }
}

fn normalize_import_targets(package_dir: &Path, import_target: &Value) -> Vec<String> {
    let mut normalized = Vec::new();

    for target in collect_import_targets(import_target) {
        if let Some(comparable) = normalize_comparable_target(package_dir, &target) {
            if !normalized.contains(&comparable) {
                normalized.push(comparable);
            }
        }
    }

    normalized
}

fn collect_import_targets(import_target: &Value) -> Vec<String> {
    match import_target {
        Value::String(value) => vec![value.clone()],
        Value::Object(object) => object.values().flat_map(collect_import_targets).collect(),
        _ => Vec::new(),
    }
}

fn normalize_comparable_target(base_dir: &Path, target: &str) -> Option<String> {
    if target.is_empty() {
        return None;
    }

    Some(normalize_path_for_match(&resolve_path(base_dir, target)))
}

fn stringify_import_target(import_target: &Value) -> String {
    import_target
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| serde_json::to_string(import_target).unwrap_or_default())
}

fn alias_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    if has_url_like_prefix(target) || target.starts_with('@') {
        return Ok(true);
    }

    if target.contains('*') {
        return wildcard_target_exists(base_dir, target);
    }

    static_target_exists(base_dir, target)
}

fn static_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    let stem = target.split('*').next().unwrap_or(target);
    let resolved = resolve_path(base_dir, stem);

    if path_exists(&resolved) {
        return Ok(true);
    }

    let resolved_string = resolved.to_string_lossy();
    for extension in CHECKABLE_EXTENSIONS {
        if path_exists(PathBuf::from(format!("{resolved_string}{extension}"))) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn wildcard_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    let mut segments = target.split('*');
    let prefix = segments.next().unwrap_or_default();
    let rest = segments.collect::<Vec<_>>();
    let suffix = rest.join("*");
    let search_prefix = wildcard_search_prefix(prefix);
    let search_root = resolve_path(base_dir, search_prefix);

    if !path_exists(&search_root) {
        return Ok(false);
    }

    if rest.len() == 1 && suffix.is_empty() {
        return Ok(true);
    }

    let patterns = build_wildcard_patterns(base_dir, target);
    has_matching_path(&search_root, &patterns)
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

fn has_matching_path(candidate_path: &Path, patterns: &[String]) -> io::Result<bool> {
    if matches_path(candidate_path, patterns) {
        return Ok(true);
    }

    let entries = match fs::read_dir(candidate_path) {
        Ok(entries) => entries,
        Err(_) => return Ok(false),
    };

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();

        if matches_path(&entry_path, patterns) {
            return Ok(true);
        }

        if entry.file_type()?.is_dir() && has_matching_path(&entry_path, patterns)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn matches_path(candidate_path: &Path, patterns: &[String]) -> bool {
    let normalized_path = normalize_path_for_match(candidate_path);
    patterns
        .iter()
        .any(|pattern| wildcard_pattern_matches(&normalized_path, pattern))
}

fn build_wildcard_patterns(base_dir: &Path, target: &str) -> Vec<String> {
    let resolved = resolve_path(base_dir, target);
    let mut patterns = vec![normalize_path_for_match(&resolved)];

    if !has_explicit_extension(target) {
        let resolved_string = resolved.to_string_lossy();
        for extension in CHECKABLE_EXTENSIONS {
            patterns.push(normalize_path_for_match(&PathBuf::from(format!(
                "{resolved_string}{extension}"
            ))));
        }
    }

    patterns
}

fn has_explicit_extension(target: &str) -> bool {
    let tail = target.rsplit('*').next().unwrap_or(target);
    Path::new(tail).extension().is_some()
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

fn has_url_like_prefix(target: &str) -> bool {
    let Some((prefix, _)) = target.split_once(':') else {
        return false;
    };

    !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn resolve_path(base_dir: &Path, target: &str) -> PathBuf {
    normalize_path(&base_dir.join(target))
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
