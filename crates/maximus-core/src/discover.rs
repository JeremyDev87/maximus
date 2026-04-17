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

pub fn discover_project(root_dir: impl AsRef<Path>) -> io::Result<ProjectSnapshot> {
    let root_dir = root_dir.as_ref().to_path_buf();
    let mut files = Vec::new();

    let walker = WalkDir::new(&root_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit(entry));

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

fn should_visit(entry: &DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return true;
    }

    !IGNORED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref())
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
