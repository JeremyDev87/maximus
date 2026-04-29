use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{
    is_ignored_project_path_from_root, make_finding, FindingInput, ProjectSnapshot, Severity,
};

use crate::check_outcome::CheckOutcome;

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

const ARTIFACT_PATTERNS: &[ArtifactPattern] = &[
    ArtifactPattern {
        canonical: "node_modules",
        aliases: &["node_modules"],
    },
    ArtifactPattern {
        canonical: "coverage",
        aliases: &["coverage"],
    },
    ArtifactPattern {
        canonical: "dist",
        aliases: &["dist"],
    },
    ArtifactPattern {
        canonical: "build",
        aliases: &["build"],
    },
    ArtifactPattern {
        canonical: "out",
        aliases: &["out"],
    },
    ArtifactPattern {
        canonical: "target",
        aliases: &["target"],
    },
    ArtifactPattern {
        canonical: "tmp",
        aliases: &["tmp", ".tmp"],
    },
    ArtifactPattern {
        canonical: "*.tgz",
        aliases: &["*.tgz"],
    },
];

struct ArtifactPattern {
    canonical: &'static str,
    aliases: &'static [&'static str],
}

#[derive(Debug, Default)]
struct DirectoryIgnoreFiles {
    gitignore: Option<ParsedIgnoreFile>,
    maximusignore: Option<ParsedIgnoreFile>,
}

#[derive(Debug)]
struct ParsedIgnoreFile {
    path: PathBuf,
    patterns: BTreeSet<String>,
}

pub fn run_ignore_file_drift_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    run_ignore_file_drift_check_with_ignore_root(project, &[], &project.root_dir)
}

pub(crate) fn run_ignore_file_drift_check_with_ignore_root(
    project: &ProjectSnapshot,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();
    let mut ignore_files = BTreeMap::new();
    collect_ignore_files(
        &project.root_dir,
        ignored_patterns,
        ignore_root,
        &mut ignore_files,
    )?;

    for (directory, files) in ignore_files {
        let (Some(gitignore), Some(maximusignore)) = (files.gitignore, files.maximusignore) else {
            continue;
        };

        let all_patterns = gitignore
            .patterns
            .union(&maximusignore.patterns)
            .cloned()
            .collect::<BTreeSet<_>>();

        for pattern in all_patterns {
            let in_gitignore = gitignore.patterns.contains(&pattern);
            let in_maximusignore = maximusignore.patterns.contains(&pattern);
            if in_gitignore == in_maximusignore {
                continue;
            }

            let (source, missing_file, consequence) = if in_gitignore {
                (
                    &gitignore,
                    ".maximusignore",
                    "Maximus may still scan generated or packaged artifacts that git ignores.",
                )
            } else {
                (
                    &maximusignore,
                    ".gitignore",
                    "Generated or packaged artifacts may still be committed because git does not ignore them.",
                )
            };

            findings.push(make_finding(FindingInput {
                id: format!(
                    "ignore-drift:{}:{}:{}",
                    directory.to_string_lossy(),
                    pattern,
                    missing_file
                ),
                title: "Ignore files disagree on generated artifact coverage".to_string(),
                category: Some("ignore-drift".to_string()),
                detail: Some(format!(
                    "{} is ignored in {}, but not in {} for {}. {}",
                    pattern,
                    source
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy())
                        .unwrap_or_default(),
                    missing_file,
                    relative_display_dir(&project.root_dir, &directory),
                    consequence
                )),
                file: Some(source.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Keep generated-artifact ignore entries aligned across .gitignore and .maximusignore, or remove the redundant local ignore file."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn collect_ignore_files(
    root_dir: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
    ignore_files: &mut BTreeMap<PathBuf, DirectoryIgnoreFiles>,
) -> io::Result<()> {
    if is_ignored_project_path_from_root(ignore_root, root_dir, ignored_patterns) {
        return Ok(());
    }

    let mut child_directories = Vec::new();
    let entries = match fs::read_dir(root_dir) {
        Ok(entries) => entries,
        Err(error) if should_skip_traversal_error(&error) => return Ok(()),
        Err(error) => return Err(error),
    };

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

        if file_type.is_dir() {
            if !should_ignore_directory(&entry.file_name())
                && !is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns)
            {
                child_directories.push(path);
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name != ".gitignore" && file_name != ".maximusignore" {
            continue;
        }
        if is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns) {
            continue;
        }

        let Some(text) = maximus_core::read_text_if_exists(&path)? else {
            continue;
        };
        let parsed = ParsedIgnoreFile {
            path: path.clone(),
            patterns: parse_artifact_patterns(&text),
        };
        let directory = path.parent().unwrap_or(root_dir).to_path_buf();
        let files = ignore_files.entry(directory).or_default();
        if file_name == ".gitignore" {
            files.gitignore = Some(parsed);
        } else {
            files.maximusignore = Some(parsed);
        }
    }

    child_directories.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    for child_directory in child_directories {
        collect_ignore_files(
            &child_directory,
            ignored_patterns,
            ignore_root,
            ignore_files,
        )?;
    }

    Ok(())
}

fn parse_artifact_patterns(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(normalize_ignore_line)
        .filter_map(|pattern| canonical_artifact_pattern(&pattern).map(ToOwned::to_owned))
        .collect()
}

fn normalize_ignore_line(line: &str) -> Option<String> {
    let mut value = line.trim_end().to_string();
    if value.is_empty() || value.starts_with('#') || value.starts_with('!') {
        return None;
    }
    if value.starts_with("\\!") {
        value.replace_range(..2, "!");
    } else if value.starts_with("\\#") {
        value.replace_range(..2, "#");
    }

    let normalized = value
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if normalized.is_empty() || normalized.contains('/') {
        return None;
    }

    Some(normalized)
}

fn canonical_artifact_pattern(pattern: &str) -> Option<&'static str> {
    ARTIFACT_PATTERNS
        .iter()
        .find(|artifact| artifact.aliases.contains(&pattern))
        .map(|artifact| artifact.canonical)
}

fn should_ignore_directory(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .map(|value| IGNORED_DIRECTORIES.contains(&value))
        .unwrap_or(false)
}

fn relative_display_dir(root_dir: &Path, directory: &Path) -> String {
    directory
        .strip_prefix(root_dir)
        .map(|relative| {
            let relative = relative.to_string_lossy().replace('\\', "/");
            if relative.is_empty() {
                ".".to_string()
            } else {
                relative
            }
        })
        .unwrap_or_else(|_| directory.to_string_lossy().to_string())
}

fn should_skip_traversal_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}
