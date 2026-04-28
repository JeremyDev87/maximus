use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{parse_jsonc, read_text_if_exists, ParseJsoncError};

const CONFIG_FILE_NAMES: [&str; 2] = ["maximus.config.json", ".maximusrc.json"];
const GIT_IGNORE_FILE_NAME: &str = ".gitignore";
const MAXIMUS_IGNORE_FILE_NAME: &str = ".maximusignore";
const DEFAULT_IGNORE_PATTERN_DIRS: &[&str] = &[
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaximusConfig {
    #[serde(default)]
    pub checks: CheckFilterConfig,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "ignorePatterns")]
    pub ignore_patterns: Vec<String>,
    #[serde(skip)]
    pub gitignore_patterns: Vec<String>,
    #[serde(default)]
    pub severity: BTreeMap<String, ConfigSeverity>,
    #[serde(default)]
    pub suppressions: Vec<ConfigSuppression>,
    #[serde(default)]
    pub report: ReportConfig,
}

impl MaximusConfig {
    pub fn effective_ignore_patterns(&self) -> Vec<String> {
        let mut patterns = self.gitignore_patterns.clone();
        patterns.extend(self.non_git_ignore_patterns());
        patterns
    }

    pub fn non_git_ignore_patterns(&self) -> Vec<String> {
        let mut patterns = self.ignore.clone();
        patterns.extend(self.ignore_patterns.clone());
        patterns
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IgnoreFilePatternSources {
    pub maximusignore: Vec<String>,
    pub gitignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IgnoreFileKind {
    Maximus,
    Git,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckFilterConfig {
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSuppression {
    pub id: String,
    #[serde(default, rename = "filePrefix")]
    pub file_prefix: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSeverity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailOnLevel {
    Error,
    Warn,
    Info,
    None,
}

impl Default for FailOnLevel {
    fn default() -> Self {
        Self::Warn
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportConfig {
    #[serde(default, rename = "failOn")]
    pub fail_on: Option<FailOnLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: MaximusConfig,
}

#[derive(Debug)]
pub enum LoadConfigError {
    Io(io::Error),
    Parse(ParseJsoncError),
}

impl std::fmt::Display for LoadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LoadConfigError {}

impl From<io::Error> for LoadConfigError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ParseJsoncError> for LoadConfigError {
    fn from(value: ParseJsoncError) -> Self {
        Self::Parse(value)
    }
}

pub fn load_maximus_config(start_dir: &Path) -> Result<Option<LoadedConfig>, LoadConfigError> {
    let Some(config_path) = find_maximus_config_path(start_dir)? else {
        return Ok(None);
    };
    let Some(text) = read_text_if_exists(&config_path)? else {
        return Ok(None);
    };
    let config = parse_jsonc::<MaximusConfig>(&text, &config_path.to_string_lossy())?;

    Ok(Some(LoadedConfig {
        path: config_path,
        config,
    }))
}

pub fn find_maximus_config_path(start_dir: &Path) -> io::Result<Option<PathBuf>> {
    let start_dir = std::path::absolute(start_dir)?;
    let mut start_dirs = Vec::new();
    if let Ok(canonical_start_dir) = std::fs::canonicalize(&start_dir) {
        start_dirs.push(canonical_start_dir);
    }
    start_dirs.push(start_dir.clone());
    let mut searched_directories = BTreeSet::new();

    for start_dir in start_dirs {
        for directory in start_dir.ancestors() {
            if !searched_directories.insert(directory.to_path_buf()) {
                continue;
            }
            for file_name in CONFIG_FILE_NAMES {
                let candidate = directory.join(file_name);
                if candidate.is_file() {
                    return Ok(Some(candidate));
                }
            }
        }
    }

    Ok(None)
}

pub fn find_ignore_root(start_dir: &Path, config_path: Option<&Path>) -> io::Result<PathBuf> {
    let start_dir = std::path::absolute(start_dir)?;
    let mut start_dirs = Vec::new();
    if let Ok(canonical_start_dir) = std::fs::canonicalize(&start_dir) {
        start_dirs.push(canonical_start_dir);
    }
    start_dirs.push(start_dir.clone());
    let mut searched_directories = BTreeSet::new();

    for start_dir in start_dirs {
        for directory in start_dir.ancestors() {
            if !searched_directories.insert(directory.to_path_buf()) {
                continue;
            }
            if directory.join(".git").exists() {
                return Ok(directory.to_path_buf());
            }
        }
    }

    if let Some(config_path) = config_path {
        if let Some(parent) = config_path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    Ok(start_dir)
}

pub fn scope_ignore_patterns(
    patterns: &[String],
    base_dir: &Path,
    ignore_root: &Path,
) -> io::Result<Vec<String>> {
    let base_dir = canonical_or_absolute(base_dir)?;
    let ignore_root = canonical_or_absolute(ignore_root)?;
    let base_prefix = base_dir
        .strip_prefix(ignore_root)
        .ok()
        .map(relative_path_string)
        .filter(|value| value != ".")
        .unwrap_or_default();

    Ok(patterns
        .iter()
        .filter_map(|pattern| normalize_ignore_file_line(pattern, &base_prefix))
        .collect())
}

pub fn load_ignore_file_patterns(ignore_root: &Path, target_dir: &Path) -> io::Result<Vec<String>> {
    let sources = load_ignore_file_pattern_sources(ignore_root, target_dir)?;
    let mut patterns = sources.gitignore;
    patterns.extend(sources.maximusignore);

    Ok(patterns)
}

pub fn load_ignore_file_pattern_sources(
    ignore_root: &Path,
    target_dir: &Path,
) -> io::Result<IgnoreFilePatternSources> {
    let ignore_root = canonical_or_absolute(ignore_root)?;
    let target_dir = canonical_or_absolute(target_dir)?;
    let include_gitignore = ignore_root.join(".git").exists();
    let mut ignore_files = BTreeMap::new();

    for directory in ignore_directories_from_root_to_target(&ignore_root, &target_dir) {
        insert_ignore_files(&mut ignore_files, &directory, include_gitignore);
    }

    if target_dir.starts_with(&ignore_root) {
        for entry in WalkDir::new(&target_dir)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| should_visit_for_ignore_files(entry))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if should_skip_walk_error(&error) => continue,
                Err(error) => return Err(error.into()),
            };

            if entry.depth() == 0 || !entry.file_type().is_file() {
                continue;
            }
            if let Some(kind) = ignore_file_kind(entry.file_name(), include_gitignore) {
                ignore_files.insert(entry.path().to_path_buf(), kind);
            }
        }
    }

    let mut sources = IgnoreFilePatternSources::default();
    for (ignore_file, kind) in ignore_files {
        let Some(text) = read_text_if_exists(&ignore_file)? else {
            continue;
        };
        let base_dir = ignore_file.parent().unwrap_or(ignore_root.as_path());
        let patterns = parse_ignore_file_patterns(&text, base_dir, &ignore_root);
        match kind {
            IgnoreFileKind::Maximus => sources.maximusignore.extend(patterns),
            IgnoreFileKind::Git => sources.gitignore.extend(patterns),
        }
    }

    Ok(sources)
}

fn canonical_or_absolute(path: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(path).or_else(|_| std::path::absolute(path))
}

fn ignore_directories_from_root_to_target(ignore_root: &Path, target_dir: &Path) -> Vec<PathBuf> {
    if !target_dir.starts_with(ignore_root) {
        return vec![ignore_root.to_path_buf()];
    }

    let mut directories = Vec::new();
    let mut current = Some(target_dir);
    while let Some(directory) = current {
        directories.push(directory.to_path_buf());
        if directory == ignore_root {
            break;
        }
        current = directory.parent();
    }
    directories.reverse();
    directories
}

fn insert_ignore_files(
    ignore_files: &mut BTreeMap<PathBuf, IgnoreFileKind>,
    directory: &Path,
    include_gitignore: bool,
) {
    let maximusignore = directory.join(MAXIMUS_IGNORE_FILE_NAME);
    if maximusignore.is_file() {
        ignore_files.insert(maximusignore, IgnoreFileKind::Maximus);
    }

    if include_gitignore {
        let gitignore = directory.join(GIT_IGNORE_FILE_NAME);
        if gitignore.is_file() {
            ignore_files.insert(gitignore, IgnoreFileKind::Git);
        }
    }
}

fn ignore_file_kind(file_name: &OsStr, include_gitignore: bool) -> Option<IgnoreFileKind> {
    if file_name == MAXIMUS_IGNORE_FILE_NAME {
        return Some(IgnoreFileKind::Maximus);
    }
    if include_gitignore && file_name == GIT_IGNORE_FILE_NAME {
        return Some(IgnoreFileKind::Git);
    }

    None
}

fn parse_ignore_file_patterns(text: &str, base_dir: &Path, ignore_root: &Path) -> Vec<String> {
    text.lines()
        .filter_map(|line| normalize_ignore_file_line_with_paths(line, base_dir, ignore_root))
        .collect()
}

fn normalize_ignore_file_line_with_paths(
    line: &str,
    base_dir: &Path,
    ignore_root: &Path,
) -> Option<String> {
    let base_prefix = base_dir
        .strip_prefix(ignore_root)
        .ok()
        .map(relative_path_string)
        .filter(|value| value != ".")
        .unwrap_or_default();

    normalize_ignore_file_line(line, &base_prefix)
}

fn normalize_ignore_file_line(line: &str, base_prefix: &str) -> Option<String> {
    let mut value = line.trim_end();
    if value.is_empty() || value.starts_with('#') {
        return None;
    }

    let mut negated = false;
    let mut literal_prefix = None;
    if let Some(remainder) = value.strip_prefix('!') {
        negated = true;
        value = remainder;
    } else if let Some(remainder) = value.strip_prefix("\\!") {
        literal_prefix = Some('!');
        value = remainder;
    } else if let Some(remainder) = value.strip_prefix("\\#") {
        literal_prefix = Some('#');
        value = remainder;
    }

    if value.is_empty() && literal_prefix.is_none() {
        return None;
    }

    let directory_only = value.ends_with('/');
    let anchored = literal_prefix.is_none() && value.starts_with('/');
    let mut normalized = value
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if let Some(prefix) = literal_prefix {
        normalized = format!("{prefix}{normalized}");
    }

    if normalized.is_empty() {
        return None;
    }

    let scoped = if base_prefix.is_empty() {
        if anchored {
            format!("/{normalized}")
        } else {
            normalized
        }
    } else if anchored || normalized.contains('/') {
        format!("{base_prefix}/{normalized}")
    } else {
        format!("{base_prefix}/**/{normalized}")
    };
    let scoped = if directory_only {
        format!("{scoped}/")
    } else {
        scoped
    };

    Some(if negated {
        format!("!{scoped}")
    } else if scoped.starts_with('!') {
        format!("\\{scoped}")
    } else {
        scoped
    })
}

fn should_visit_for_ignore_files(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return true;
    }

    let file_name = entry.file_name().to_string_lossy();
    !DEFAULT_IGNORE_PATTERN_DIRS.contains(&file_name.as_ref())
}

fn should_skip_walk_error(error: &walkdir::Error) -> bool {
    error.depth() > 0
        && error.io_error().is_some_and(|io_error| {
            matches!(
                io_error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            )
        })
}

fn relative_path_string(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        ".".to_string()
    } else {
        value
    }
}
