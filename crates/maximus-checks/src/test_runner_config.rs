use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{
    is_ignored_project_path_from_root, make_finding, FileKind, FindingInput, ProjectFile,
    ProjectSnapshot, Severity,
};

use crate::check_outcome::CheckOutcome;
use crate::registry::{package_file_for_directory, read_package_json};

#[derive(Default)]
struct TestRunnerSources {
    jest_file: Option<PathBuf>,
    vitest_file: Option<PathBuf>,
    package_file: Option<PathBuf>,
    has_package_jest: bool,
    has_package_vitest: bool,
}

pub fn run_test_runner_config_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    run_test_runner_config_check_with_ignore_root(project, &[], &project.root_dir)
}

pub(crate) fn run_test_runner_config_check_with_ignore_root(
    project: &ProjectSnapshot,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let mut sources_by_dir = BTreeMap::<PathBuf, TestRunnerSources>::new();

    for directory in &project.directories {
        let sources = sources_by_dir.entry(directory.dir.clone()).or_default();

        if let Some(jest_file) = directory
            .files_by_kind
            .get(&FileKind::Jest)
            .and_then(|files| files.first())
        {
            sources.jest_file = Some(jest_file.path.clone());
        }

        if let Some(package_file) = package_file_for_directory(directory) {
            sources.package_file = Some(package_file.path.clone());
            sources.has_package_jest = package_has_jest_config(package_file);
            sources.has_package_vitest = package_has_vitest_config(package_file);
        }
    }

    for vitest_file in find_vitest_config_files(&project.root_dir, ignored_patterns, ignore_root)? {
        let directory = vitest_file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project.root_dir.clone());
        sources_by_dir.entry(directory).or_default().vitest_file = Some(vitest_file);
    }

    let mut findings = Vec::new();
    for (directory, sources) in sources_by_dir {
        let has_jest = sources.jest_file.is_some() || sources.has_package_jest;
        let has_vitest = sources.vitest_file.is_some() || sources.has_package_vitest;

        if !has_jest || !has_vitest {
            continue;
        }

        let file = sources.jest_file.or(sources.package_file);
        findings.push(make_finding(FindingInput {
            id: format!("test-runner-dual-config:{}", directory.to_string_lossy()),
            title: "Jest and Vitest configs coexist".to_string(),
            category: Some("test-runner-config".to_string()),
            detail: Some(
                "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command."
                    .to_string(),
            ),
            file,
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Pick one runner for this package, or document the split with separate config ownership and scripts."
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

fn package_has_jest_config(package_file: &ProjectFile) -> bool {
    package_has_config_field(package_file, "jest")
}

fn package_has_vitest_config(package_file: &ProjectFile) -> bool {
    package_has_config_field(package_file, "vitest")
}

fn package_has_config_field(package_file: &ProjectFile, field: &str) -> bool {
    read_package_json(&package_file.path)
        .map(|package_json| {
            package_json
                .as_object()
                .map(|object| object.contains_key(field))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn find_vitest_config_files(
    root_dir: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<Vec<PathBuf>> {
    if is_ignored_project_path_from_root(ignore_root, root_dir, ignored_patterns) {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_vitest_config_files(root_dir, ignored_patterns, ignore_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_vitest_config_files(
    directory: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();

        if file_type.is_dir() {
            if should_skip_directory(&name)
                || is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns)
            {
                continue;
            }
            collect_vitest_config_files(&path, ignored_patterns, ignore_root, files)?;
        } else if file_type.is_file()
            && is_vitest_config_name(&name)
            && !is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns)
        {
            files.push(path);
        }
    }

    Ok(())
}

fn is_vitest_config_name(name: &str) -> bool {
    matches!(
        name,
        "vitest.config.js"
            | "vitest.config.cjs"
            | "vitest.config.mjs"
            | "vitest.config.ts"
            | "vitest.config.cts"
            | "vitest.config.mts"
    )
}

fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "dist" | "build" | "coverage" | "target"
    )
}
