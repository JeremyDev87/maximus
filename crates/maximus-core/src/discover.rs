use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use walkdir::{DirEntry, WalkDir};

use crate::env_parser::{is_concrete_env_file_name, is_template_env_file_name};
use crate::models::{FileKind, ProjectDirectory, ProjectFile, ProjectSnapshot};
use crate::text_order::locale_compare_like;

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".idea",
    ".next",
    ".nuxt",
    ".output",
    ".pnpm-store",
    ".svelte-kit",
    ".turbo",
    ".vercel",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "out",
    "target",
    "tmp",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum IgnorePattern {
    Component(String),
    Glob(Vec<String>),
}

pub fn discover_project(root_dir: impl AsRef<Path>) -> io::Result<ProjectSnapshot> {
    let root_dir = root_dir.as_ref().to_path_buf();
    discover_project_with_ignore(&root_dir, &[])
}

pub fn discover_project_with_ignore(
    root_dir: impl AsRef<Path>,
    ignored_patterns: &[String],
) -> io::Result<ProjectSnapshot> {
    let root_dir = root_dir.as_ref().to_path_buf();
    discover_project_with_ignore_root(&root_dir, ignored_patterns, &root_dir)
}

pub fn discover_project_with_ignore_root(
    root_dir: impl AsRef<Path>,
    ignored_patterns: &[String],
    ignore_root: impl AsRef<Path>,
) -> io::Result<ProjectSnapshot> {
    let root_dir = root_dir.as_ref().to_path_buf();
    let ignore_root = ignore_root.as_ref().to_path_buf();
    let ignored_patterns = ignored_patterns
        .iter()
        .filter_map(|pattern| normalize_ignore_pattern(pattern))
        .collect::<Vec<_>>();

    if is_ignored_path_from_root(&ignore_root, &root_dir, &ignored_patterns) {
        return Ok(ProjectSnapshot {
            root_dir,
            files: Vec::new(),
            directories: Vec::new(),
            files_by_kind: IndexMap::new(),
            package_files: Vec::new(),
        });
    }

    let mut files = Vec::new();

    let walker = WalkDir::new(&root_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit(&ignore_root, entry, &ignored_patterns));

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let Some(kind) = match_file_kind(entry.file_name().to_string_lossy().as_ref()) else {
            continue;
        };

        let path = entry.into_path();
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root_dir.clone());
        let relative_path = relative_string(&root_dir, &path);
        if is_ignored_path_from_root(&ignore_root, &path, &ignored_patterns) {
            continue;
        }

        files.push(ProjectFile {
            kind,
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            dir,
            relative_path,
        });
    }

    files.sort_by(|left, right| locale_compare_like(&left.relative_path, &right.relative_path));

    let mut directories_map: BTreeMap<PathBuf, ProjectDirectory> = BTreeMap::new();
    let mut files_by_kind: IndexMap<FileKind, Vec<ProjectFile>> = IndexMap::new();

    for file in &files {
        let directory =
            directories_map
                .entry(file.dir.clone())
                .or_insert_with(|| ProjectDirectory {
                    dir: file.dir.clone(),
                    relative_dir: relative_directory_string(&root_dir, &file.dir),
                    files: Vec::new(),
                    files_by_kind: IndexMap::new(),
                });

        directory.files.push(file.clone());
        directory
            .files_by_kind
            .entry(file.kind.clone())
            .or_default()
            .push(file.clone());
        files_by_kind
            .entry(file.kind.clone())
            .or_default()
            .push(file.clone());
    }

    let mut directories = directories_map.into_values().collect::<Vec<_>>();
    directories.sort_by(|left, right| locale_compare_like(&left.relative_dir, &right.relative_dir));

    let mut package_files = files_by_kind
        .get(&FileKind::Package)
        .cloned()
        .unwrap_or_default();
    package_files.sort_by_key(|file| {
        file.path
            .parent()
            .map(|path| path.components().count())
            .unwrap_or(0)
    });

    Ok(ProjectSnapshot {
        root_dir,
        files,
        directories,
        files_by_kind,
        package_files,
    })
}

pub fn is_ignored_project_path(
    root_dir: impl AsRef<Path>,
    target: impl AsRef<Path>,
    ignored_patterns: &[String],
) -> bool {
    is_ignored_project_path_from_root(root_dir, target, ignored_patterns)
}

pub fn is_ignored_project_path_from_root(
    ignore_root: impl AsRef<Path>,
    target: impl AsRef<Path>,
    ignored_patterns: &[String],
) -> bool {
    let ignored_patterns = ignored_patterns
        .iter()
        .filter_map(|pattern| normalize_ignore_pattern(pattern))
        .collect::<Vec<_>>();

    is_ignored_path_from_root(ignore_root.as_ref(), target.as_ref(), &ignored_patterns)
}

pub fn get_files(project: &ProjectSnapshot, kind: FileKind) -> &[ProjectFile] {
    project
        .files_by_kind
        .get(&kind)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn get_directories(project: &ProjectSnapshot) -> &[ProjectDirectory] {
    project.directories.as_slice()
}

pub fn find_nearest_package_file<'a>(
    project: &'a ProjectSnapshot,
    directory: impl AsRef<Path>,
) -> Option<&'a ProjectFile> {
    let directory = directory.as_ref();

    project.package_files.iter().rev().find(|file| {
        let package_dir = file.path.parent().unwrap_or(project.root_dir.as_path());
        directory == package_dir || directory.starts_with(package_dir)
    })
}

fn should_visit(ignore_root: &Path, entry: &DirEntry, ignored_patterns: &[IgnorePattern]) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return true;
    }

    let file_name = entry.file_name().to_string_lossy();
    if IGNORED_DIRECTORIES.contains(&file_name.as_ref()) {
        return false;
    }

    !is_ignored_path_from_root(ignore_root, entry.path(), ignored_patterns)
}

fn normalize_ignore_pattern(pattern: &str) -> Option<IgnorePattern> {
    let trimmed = pattern.trim().replace('\\', "/");
    let trimmed = trimmed.trim_start_matches("./").trim_matches('/');

    if trimmed.is_empty() {
        return None;
    }

    if !trimmed.contains('/') && !contains_glob_meta(trimmed) {
        return Some(IgnorePattern::Component(trimmed.to_string()));
    }

    Some(IgnorePattern::Glob(
        trimmed
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    ))
}

fn contains_glob_meta(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

fn is_ignored_relative_path(relative_path: &str, ignored_patterns: &[IgnorePattern]) -> bool {
    if ignored_patterns.is_empty() {
        return false;
    }

    let normalized_path = relative_path.replace('\\', "/");
    let path_segments = normalized_path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>();

    ignored_patterns.iter().any(|pattern| match pattern {
        IgnorePattern::Component(component) => path_segments
            .iter()
            .any(|segment| *segment == component.as_str()),
        IgnorePattern::Glob(pattern_segments) => {
            glob_path_matches(pattern_segments, &path_segments)
        }
    })
}

fn is_ignored_path_from_root(
    ignore_root: &Path,
    target: &Path,
    ignored_patterns: &[IgnorePattern],
) -> bool {
    if ignored_patterns.is_empty() {
        return false;
    }
    if relative_matches(ignore_root, target, ignored_patterns) {
        return true;
    }

    let Ok(canonical_ignore_root) = std::fs::canonicalize(ignore_root) else {
        return false;
    };
    let Ok(canonical_target) = std::fs::canonicalize(target) else {
        return false;
    };

    relative_matches(&canonical_ignore_root, &canonical_target, ignored_patterns)
}

fn relative_matches(ignore_root: &Path, target: &Path, ignored_patterns: &[IgnorePattern]) -> bool {
    let relative_path = relative_string(ignore_root, target);
    is_ignored_relative_path(&relative_path, ignored_patterns)
}

fn glob_path_matches(pattern_segments: &[String], path_segments: &[&str]) -> bool {
    let mut memo = vec![vec![None; path_segments.len() + 1]; pattern_segments.len() + 1];
    glob_path_matches_from(pattern_segments, path_segments, 0, 0, &mut memo)
}

fn glob_path_matches_from(
    pattern_segments: &[String],
    path_segments: &[&str],
    pattern_index: usize,
    path_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][path_index] {
        return result;
    }

    let result = if pattern_index == pattern_segments.len() {
        path_index == path_segments.len()
    } else if pattern_segments[pattern_index] == "**" {
        glob_path_matches_from(
            pattern_segments,
            path_segments,
            pattern_index + 1,
            path_index,
            memo,
        ) || (path_index < path_segments.len()
            && glob_path_matches_from(
                pattern_segments,
                path_segments,
                pattern_index,
                path_index + 1,
                memo,
            ))
    } else {
        path_index < path_segments.len()
            && glob_segment_matches(&pattern_segments[pattern_index], path_segments[path_index])
            && glob_path_matches_from(
                pattern_segments,
                path_segments,
                pattern_index + 1,
                path_index + 1,
                memo,
            )
    };

    memo[pattern_index][path_index] = Some(result);
    result
}

fn glob_segment_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; value.len() + 1]; pattern.len() + 1];

    glob_segment_matches_from(&pattern, &value, 0, 0, &mut memo)
}

fn glob_segment_matches_from(
    pattern: &[char],
    value: &[char],
    pattern_index: usize,
    value_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][value_index] {
        return result;
    }

    let result = if pattern_index == pattern.len() {
        value_index == value.len()
    } else if pattern[pattern_index] == '*' {
        glob_segment_matches_from(pattern, value, pattern_index + 1, value_index, memo)
            || (value_index < value.len()
                && glob_segment_matches_from(pattern, value, pattern_index, value_index + 1, memo))
    } else {
        value_index < value.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == value[value_index])
            && glob_segment_matches_from(pattern, value, pattern_index + 1, value_index + 1, memo)
    };

    memo[pattern_index][value_index] = Some(result);
    result
}

fn relative_string(root_dir: &Path, target: &Path) -> String {
    target
        .strip_prefix(root_dir)
        .map(|relative| {
            let value = relative.to_string_lossy().replace('\\', "/");
            if value.is_empty() {
                ".".to_string()
            } else {
                value
            }
        })
        .unwrap_or_else(|_| target.to_string_lossy().into_owned())
}

fn relative_directory_string(root_dir: &Path, target: &Path) -> String {
    let relative = relative_string(root_dir, target);
    if relative.is_empty() {
        ".".to_string()
    } else {
        relative
    }
}

fn match_file_kind(name: &str) -> Option<FileKind> {
    if name == "package.json" {
        return Some(FileKind::Package);
    }

    if name == "jsconfig.json" || is_tsconfig_file_name(name) {
        return Some(FileKind::Tsconfig);
    }

    if is_dot_config(
        name,
        ".eslintrc",
        &["json", "yaml", "yml", "js", "cjs", "mjs"],
    ) || is_named_config(
        name,
        "eslint.config",
        &["js", "cjs", "mjs", "ts", "mts", "cts"],
    ) {
        return Some(FileKind::Eslint);
    }

    if is_dot_config(
        name,
        ".prettierrc",
        &["json", "yaml", "yml", "js", "cjs", "mjs"],
    ) || name == ".prettierrc.toml"
        || is_named_config(
            name,
            "prettier.config",
            &["js", "cjs", "mjs", "ts", "mts", "cts"],
        )
    {
        return Some(FileKind::Prettier);
    }

    if is_named_config(
        name,
        "vite.config",
        &["js", "cjs", "mjs", "ts", "mts", "cts"],
    ) {
        return Some(FileKind::Vite);
    }

    if is_named_config(
        name,
        "jest.config",
        &["js", "cjs", "mjs", "ts", "mts", "cts"],
    ) {
        return Some(FileKind::Jest);
    }

    if is_named_config(
        name,
        "next.config",
        &["js", "cjs", "mjs", "ts", "mts", "cts"],
    ) {
        return Some(FileKind::Next);
    }

    if is_concrete_env_file_name(name) || is_template_env_file_name(name) {
        return Some(FileKind::Env);
    }

    if matches!(name, "pnpm-workspace.yaml" | "turbo.json") {
        return Some(FileKind::Workspace);
    }

    None
}

fn is_dot_config(name: &str, prefix: &str, extensions: &[&str]) -> bool {
    if name == prefix {
        return true;
    }

    name.strip_prefix(&format!("{prefix}."))
        .map(|extension| extensions.contains(&extension))
        .unwrap_or(false)
}

fn is_named_config(name: &str, prefix: &str, extensions: &[&str]) -> bool {
    name.strip_prefix(&format!("{prefix}."))
        .map(|extension| extensions.contains(&extension))
        .unwrap_or(false)
}

fn is_tsconfig_file_name(name: &str) -> bool {
    if name == "tsconfig.json" {
        return true;
    }

    name.strip_prefix("tsconfig.")
        .and_then(|remainder| remainder.strip_suffix(".json"))
        .map(|remainder| !remainder.is_empty())
        .unwrap_or(false)
}
