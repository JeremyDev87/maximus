use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use maximus_core::{
    get_files, is_ignored_project_path_from_root, make_finding, parse_jsonc, read_text_if_exists,
    FileKind, FindingInput, ProjectSnapshot, Severity,
};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;

pub fn run_npmignore_files_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    run_npmignore_files_check_with_ignore_root(project, &[], &project.root_dir)
}

pub(crate) fn run_npmignore_files_check_with_ignore_root(
    project: &ProjectSnapshot,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for package_file in get_files(project, FileKind::Package) {
        if is_ignored_project_path_from_root(ignore_root, &package_file.path, ignored_patterns) {
            continue;
        }
        let Some(files_entries) = read_package_files_entries(&package_file.path)? else {
            continue;
        };
        if files_entries.is_empty() {
            continue;
        }

        let package_dir = package_file
            .path
            .parent()
            .unwrap_or(project.root_dir.as_path());
        let mut seen_finding_ids = BTreeSet::new();

        for files_entry in files_entries {
            let Some(normalized_entry) = normalize_files_entry(&files_entry) else {
                continue;
            };
            collect_nested_npmignore_findings(
                &mut findings,
                &mut seen_finding_ids,
                &package_file.path,
                package_dir,
                &files_entry,
                &normalized_entry,
                ignored_patterns,
                ignore_root,
            )?;
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

#[derive(Debug)]
struct NpmignorePattern {
    raw: String,
    normalized: String,
    negated: bool,
    directory_only: bool,
    anchored: bool,
}

fn collect_nested_npmignore_findings(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    normalized_entry: &str,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    let included_path = if normalized_entry == "." {
        package_dir.to_path_buf()
    } else {
        package_dir.join(normalized_entry)
    };
    if contains_glob(normalized_entry) {
        collect_glob_nested_npmignore_findings(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            normalized_entry,
            ignored_patterns,
            ignore_root,
        )?;
        return Ok(());
    }

    if is_ignored_project_path_from_root(ignore_root, &included_path, ignored_patterns) {
        return Ok(());
    }

    if included_path.is_file() {
        collect_file_nested_npmignore_findings(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &included_path,
            ignored_patterns,
            ignore_root,
        )?;
        return Ok(());
    }

    if included_path.is_dir() {
        collect_directory_nested_npmignore_findings(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &included_path,
            ignored_patterns,
            ignore_root,
        )?;
    }

    Ok(())
}

fn collect_glob_nested_npmignore_findings(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    normalized_entry: &str,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    let mut candidate_files = Vec::new();
    collect_candidate_files(package_dir, &mut candidate_files)?;

    for file_path in candidate_files {
        if is_ignored_project_path_from_root(ignore_root, &file_path, ignored_patterns) {
            continue;
        }
        let Some(relative_to_package) = relative_path(package_dir, &file_path) else {
            continue;
        };
        if !files_entry_includes_file(normalized_entry, &relative_to_package) {
            continue;
        }

        collect_file_nested_npmignore_findings(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &file_path,
            ignored_patterns,
            ignore_root,
        )?;
    }

    Ok(())
}

fn collect_file_nested_npmignore_findings(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    file_path: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    if is_ignored_project_path_from_root(ignore_root, file_path, ignored_patterns) {
        return Ok(());
    }

    let mut directories = Vec::new();
    let mut current = file_path.parent();
    while let Some(directory) = current {
        if directory == package_dir {
            break;
        }
        directories.push(directory.to_path_buf());
        current = directory.parent();
    }
    directories.reverse();

    for directory in directories {
        let npmignore_path = directory.join(".npmignore");
        collect_npmignore_findings_for_file(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &npmignore_path,
            file_path,
            ignored_patterns,
            ignore_root,
        )?;
    }

    Ok(())
}

fn collect_directory_nested_npmignore_findings(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    directory: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if should_skip_traversal_error(&error) => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut child_directories = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if should_skip_traversal_error(&error) => continue,
            Err(error) => return Err(error),
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if should_skip_traversal_error(&error) => continue,
            Err(error) => return Err(error),
        };
        let path = entry.path();

        if is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns) {
            continue;
        }

        if file_type.is_dir() {
            child_directories.push(path);
            continue;
        }

        if !file_type.is_file()
            || path.file_name().and_then(|name| name.to_str()) != Some(".npmignore")
        {
            continue;
        }
        if path.parent() == Some(package_dir) {
            continue;
        }

        collect_npmignore_findings_for_directory(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &path,
            ignored_patterns,
            ignore_root,
        )?;
    }

    child_directories.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    for child_directory in child_directories {
        collect_directory_nested_npmignore_findings(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            &child_directory,
            ignored_patterns,
            ignore_root,
        )?;
    }

    Ok(())
}

fn collect_npmignore_findings_for_directory(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    npmignore_path: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    let Some(npmignore_dir) = npmignore_path.parent() else {
        return Ok(());
    };
    let mut candidate_files = Vec::new();
    collect_candidate_files(npmignore_dir, &mut candidate_files)?;
    for file_path in candidate_files {
        if is_ignored_project_path_from_root(ignore_root, &file_path, ignored_patterns) {
            continue;
        }
        collect_npmignore_findings_for_file(
            findings,
            seen_finding_ids,
            package_path,
            package_dir,
            files_entry,
            npmignore_path,
            &file_path,
            ignored_patterns,
            ignore_root,
        )?;
    }

    Ok(())
}

fn collect_npmignore_findings_for_file(
    findings: &mut Vec<maximus_core::Finding>,
    seen_finding_ids: &mut BTreeSet<String>,
    package_path: &Path,
    package_dir: &Path,
    files_entry: &str,
    npmignore_path: &Path,
    file_path: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<()> {
    if !npmignore_path.is_file() {
        return Ok(());
    }
    if is_ignored_project_path_from_root(ignore_root, file_path, ignored_patterns) {
        return Ok(());
    }
    if file_path.file_name().and_then(|name| name.to_str()) == Some(".npmignore") {
        return Ok(());
    }

    let Some(npmignore_dir) = npmignore_path.parent() else {
        return Ok(());
    };
    let Some(relative_to_npmignore) = relative_path(npmignore_dir, file_path) else {
        return Ok(());
    };
    let Some(relative_to_package) = relative_path(package_dir, file_path) else {
        return Ok(());
    };
    let Some(npmignore_text) = read_text_if_exists(npmignore_path)? else {
        return Ok(());
    };
    let patterns = parse_npmignore_patterns(&npmignore_text);
    let Some(pattern) = final_ignoring_pattern(&relative_to_npmignore, &patterns) else {
        return Ok(());
    };

    let id = format!(
        "npmignore-files:{}:{}",
        package_path.to_string_lossy(),
        relative_to_package
    );
    if !seen_finding_ids.insert(id.clone()) {
        return Ok(());
    }

    findings.push(make_finding(FindingInput {
        id,
        title: "package.json files entry is excluded by nested .npmignore".to_string(),
        category: Some("npmignore-files".to_string()),
        detail: Some(format!(
            "package.json files includes {files_entry:?}, but nested .npmignore pattern {:?} excludes {:?}.",
            pattern.raw, relative_to_package
        )),
        file: Some(npmignore_path.to_path_buf()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some(
            "Remove the nested .npmignore rule or adjust package.json files so publish intent is unambiguous."
                .to_string(),
        ),
        severity: Some(Severity::Warn),
    }));

    Ok(())
}

fn collect_candidate_files(
    directory: &Path,
    candidate_files: &mut Vec<std::path::PathBuf>,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if should_skip_traversal_error(&error) => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut child_directories = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if should_skip_traversal_error(&error) => continue,
            Err(error) => return Err(error),
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if should_skip_traversal_error(&error) => continue,
            Err(error) => return Err(error),
        };
        let path = entry.path();

        if file_type.is_file() {
            candidate_files.push(path);
        } else if file_type.is_dir() {
            child_directories.push(path);
        }
    }

    child_directories.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    for child_directory in child_directories {
        collect_candidate_files(&child_directory, candidate_files)?;
    }

    Ok(())
}

fn relative_path(base: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(base)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

fn should_skip_traversal_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

fn read_package_files_entries(package_path: &Path) -> io::Result<Option<Vec<String>>> {
    let Some(text) = read_text_if_exists(package_path)? else {
        return Ok(None);
    };
    let Ok(package_json) = parse_jsonc::<Value>(&text, &package_path.to_string_lossy()) else {
        return Ok(None);
    };
    let Some(files) = package_json.get("files").and_then(Value::as_array) else {
        return Ok(None);
    };

    Ok(Some(
        files
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    ))
}

fn parse_npmignore_patterns(text: &str) -> Vec<NpmignorePattern> {
    text.lines()
        .filter_map(|line| {
            let mut value = line.trim_end().to_string();
            if value.is_empty() || value.starts_with('#') {
                return None;
            }

            let negated = if value.starts_with("\\!") {
                value.replace_range(..2, "!");
                false
            } else if value.starts_with("\\#") {
                value.replace_range(..2, "#");
                false
            } else if value.starts_with('!') {
                value.remove(0);
                true
            } else {
                false
            };

            let directory_only = value.ends_with('/');
            let anchored = value.starts_with('/');
            let normalized = value
                .replace('\\', "/")
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string();
            if normalized.is_empty() {
                return None;
            }

            Some(NpmignorePattern {
                raw: line.trim_end().to_string(),
                normalized,
                negated,
                directory_only,
                anchored,
            })
        })
        .collect()
}

fn final_ignoring_pattern<'a>(
    files_entry: &str,
    patterns: &'a [NpmignorePattern],
) -> Option<&'a NpmignorePattern> {
    let mut ignored_by = None;
    for pattern in patterns {
        if pattern_matches_files_entry(pattern, files_entry) {
            if pattern.negated {
                ignored_by = None;
            } else {
                ignored_by = Some(pattern);
            }
        }
    }

    ignored_by
}

fn pattern_matches_files_entry(pattern: &NpmignorePattern, files_entry: &str) -> bool {
    let pattern_value = pattern.normalized.as_str();
    if pattern.directory_only {
        return directory_only_pattern_matches(pattern_value, files_entry, pattern.anchored);
    }
    if pattern_value == files_entry {
        return true;
    }
    if !pattern_value.contains('/') {
        if pattern.anchored {
            return glob_matches(pattern_value, files_entry)
                || ancestor_directory_paths(files_entry)
                    .iter()
                    .any(|ancestor| glob_matches(pattern_value, ancestor));
        }
        return path_component_matches(files_entry, pattern_value);
    }
    if !pattern_value.contains('*') && path_starts_with(files_entry, pattern_value) {
        return true;
    }

    glob_matches(pattern_value, files_entry)
}

fn directory_only_pattern_matches(pattern: &str, file_path: &str, anchored: bool) -> bool {
    let ancestors = ancestor_directory_paths(file_path);
    if anchored {
        return ancestors
            .iter()
            .any(|ancestor| glob_matches(pattern, ancestor));
    }
    if !pattern.contains('/') {
        return ancestors.iter().any(|ancestor| {
            ancestor
                .rsplit('/')
                .next()
                .is_some_and(|component| segment_glob_matches(pattern, component))
        });
    }

    ancestors
        .iter()
        .any(|ancestor| glob_matches(pattern, ancestor))
}

fn normalize_files_entry(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();

    if normalized.is_empty() || normalized.starts_with('!') {
        None
    } else {
        Some(normalized)
    }
}

fn path_starts_with(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn path_component_matches(path: &str, pattern: &str) -> bool {
    path.split('/')
        .any(|component| segment_glob_matches(pattern, component))
}

fn ancestor_directory_paths(path: &str) -> Vec<String> {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() <= 1 {
        return Vec::new();
    }

    let mut ancestors = Vec::new();
    let mut current = String::new();
    for segment in &segments[..segments.len() - 1] {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        ancestors.push(current.clone());
    }

    ancestors
}

fn contains_glob(value: &str) -> bool {
    value.contains('*') || value.contains('?')
}

fn files_entry_includes_file(pattern: &str, path: &str) -> bool {
    if contains_glob(pattern) {
        glob_matches(pattern, path)
            || ancestor_directory_paths(path)
                .iter()
                .any(|ancestor| glob_matches(pattern, ancestor))
    } else {
        path_starts_with(path, pattern)
    }
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let path_segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    glob_segments_match(&pattern_segments, &path_segments)
}

fn glob_segments_match(pattern_segments: &[&str], path_segments: &[&str]) -> bool {
    let Some((pattern_head, remaining_patterns)) = pattern_segments.split_first() else {
        return path_segments.is_empty();
    };

    if *pattern_head == "**" {
        if glob_segments_match(remaining_patterns, path_segments) {
            return true;
        }
        return path_segments
            .split_first()
            .is_some_and(|(_, remaining_paths)| {
                glob_segments_match(pattern_segments, remaining_paths)
            });
    }

    let Some((path_head, remaining_paths)) = path_segments.split_first() else {
        return false;
    };
    if !segment_glob_matches(pattern_head, path_head) {
        return false;
    }

    glob_segments_match(remaining_patterns, remaining_paths)
}

fn segment_glob_matches(pattern: &str, value: &str) -> bool {
    if !contains_glob(pattern) {
        return pattern == value;
    }

    fn matches(
        pattern_index: usize,
        value_index: usize,
        pattern: &[char],
        value: &[char],
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(result) = memo[pattern_index][value_index] {
            return result;
        }

        let result = if pattern_index == pattern.len() {
            value_index == value.len()
        } else if pattern[pattern_index] == '*' {
            (value_index..=value.len()).any(|next_value_index| {
                matches(pattern_index + 1, next_value_index, pattern, value, memo)
            })
        } else if pattern[pattern_index] == '?' {
            value_index < value.len()
                && matches(pattern_index + 1, value_index + 1, pattern, value, memo)
        } else {
            value_index < value.len()
                && pattern[pattern_index] == value[value_index]
                && matches(pattern_index + 1, value_index + 1, pattern, value, memo)
        };

        memo[pattern_index][value_index] = Some(result);
        result
    }

    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let value_chars = value.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; value_chars.len() + 1]; pattern_chars.len() + 1];

    matches(0, 0, &pattern_chars, &value_chars, &mut memo)
}
