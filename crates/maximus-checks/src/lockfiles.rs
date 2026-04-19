use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{make_finding, FindingInput, ProjectSnapshot, Severity};

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

const KNOWN_LOCKFILES: &[&str] = &[
    "bun.lock",
    "bun.lockb",
    "npm-shrinkwrap.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
];

pub fn run_lockfiles_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();
    let mut lockfiles_by_directory = BTreeMap::new();

    collect_lockfiles(&project.root_dir, &mut lockfiles_by_directory)?;

    for (directory, mut lockfiles) in lockfiles_by_directory {
        if lockfiles.len() <= 1 {
            continue;
        }

        lockfiles.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
        let lockfile_names = lockfiles
            .iter()
            .filter_map(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
            .collect::<Vec<_>>();

        findings.push(make_finding(FindingInput {
            id: format!("lockfiles:multiple:{}", directory.to_string_lossy()),
            title: "Multiple lockfiles are present in one directory".to_string(),
            category: Some("lockfiles".to_string()),
            detail: Some(format!(
                "Found {} known lockfiles in {}: {}.",
                lockfile_names.len(),
                relative_display_dir(&project.root_dir, &directory),
                lockfile_names.join(", ")
            )),
            file: Some(lockfiles[0].clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Keep one lockfile per directory so dependency resolution stays predictable. "
                    .to_string()
                    + "Separate package directories can each have their own lockfile.",
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

fn collect_lockfiles(
    root_dir: &Path,
    lockfiles_by_directory: &mut BTreeMap<PathBuf, Vec<PathBuf>>,
) -> io::Result<()> {
    visit_directory(root_dir, lockfiles_by_directory)
}

fn visit_directory(
    directory: &Path,
    lockfiles_by_directory: &mut BTreeMap<PathBuf, Vec<PathBuf>>,
) -> io::Result<()> {
    let mut child_directories = Vec::new();

    let entries = match fs::read_dir(directory) {
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
            if !should_ignore_directory(&entry.file_name()) {
                child_directories.push(path);
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if KNOWN_LOCKFILES.contains(&name) {
            lockfiles_by_directory
                .entry(directory.to_path_buf())
                .or_default()
                .push(path);
        }
    }

    child_directories.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    for child_directory in child_directories {
        if let Err(error) = visit_directory(&child_directory, lockfiles_by_directory) {
            if should_skip_traversal_error(&error) {
                continue;
            }
            return Err(error);
        }
    }

    Ok(())
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
