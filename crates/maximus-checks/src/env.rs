use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use maximus_core::{
    is_concrete_env_file_name, is_template_env_file_name, looks_like_secret, make_finding,
    parse_env, plan_create_env_example, plan_create_env_example_with_groups, plan_sync_env_example,
    plan_sync_env_example_with_groups, read_text_if_exists, render_env_template,
    render_env_template_groups, sort_findings, unique_fixes, EnvTemplateRenderOptions,
    EnvTemplateSourceGroup, FileKind, FindingInput, FixPlan, ProjectFile, ProjectSnapshot,
    Severity,
};

use crate::check_outcome::CheckOutcome;

pub fn run_env_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    run_env_check_with_options(project, &EnvCheckOptions::default())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvCheckOptions {
    pub template_render: EnvTemplateRenderOptions,
}

pub fn run_env_check_with_options(
    project: &ProjectSnapshot,
    options: &EnvCheckOptions,
) -> io::Result<CheckOutcome> {
    run_env_check_with_missing_concrete_excluded_keys(project, options, &BTreeSet::new())
}

pub fn run_env_check_with_missing_concrete_excluded_keys(
    project: &ProjectSnapshot,
    options: &EnvCheckOptions,
    missing_concrete_excluded_keys: &BTreeSet<String>,
) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();
    let mut fixes = Vec::new();
    let mut planned_fixes = Vec::new();
    let gitignore_traversal_root = find_gitignore_traversal_root(&project.root_dir);

    for directory in &project.directories {
        let env_files = directory
            .files_by_kind
            .get(&FileKind::Env)
            .cloned()
            .unwrap_or_default();
        if env_files.is_empty() {
            continue;
        }

        let mut parsed_records = Vec::new();
        for file in env_files {
            let Some(text) = read_text_if_exists(&file.path)? else {
                continue;
            };

            let parsed = parse_env(&text, Some(&file.name));
            parsed_records.push(ParsedEnvRecord {
                file,
                parsed_source_text: text,
                parsed,
            });
        }

        if parsed_records.is_empty() {
            continue;
        }

        for record in &parsed_records {
            for duplicate in &record.parsed.duplicates {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "env-duplicate:{}:{}:{}",
                        record.file.path.to_string_lossy(),
                        duplicate.key,
                        duplicate.second_line
                    ),
                    title: format!("Duplicate env key \"{}\"", duplicate.key),
                    category: Some("env".to_string()),
                    detail: Some(format!(
                        "{} is declared on lines {} and {}.",
                        duplicate.key, duplicate.first_line, duplicate.second_line
                    )),
                    file: Some(record.file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Keep one declaration per env file so overrides stay explicit.".to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
            }

            for invalid_line in &record.parsed.invalid_lines {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "env-invalid:{}:{}",
                        record.file.path.to_string_lossy(),
                        invalid_line.line
                    ),
                    title: "Invalid env syntax".to_string(),
                    category: Some("env".to_string()),
                    detail: Some(format!(
                        "Line {} could not be parsed as KEY=value.",
                        invalid_line.line
                    )),
                    file: Some(record.file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Use shell-style env syntax or move comments to their own line."
                            .to_string(),
                    ),
                    severity: Some(Severity::Warn),
                }));
            }
        }

        let mut contract_records = parsed_records
            .iter()
            .filter(|record| is_template_env_file_name(&record.file.name))
            .collect::<Vec<_>>();
        contract_records.sort_by(|left, right| {
            score_contract_record(&left.file.name)
                .cmp(&score_contract_record(&right.file.name))
                .then_with(|| left.file.name.cmp(&right.file.name))
        });

        let example_record = contract_records.first().copied();
        let concrete_records = parsed_records
            .iter()
            .filter(|record| is_concrete_env_file_name(&record.file.name))
            .collect::<Vec<_>>();
        let sync_target_keys = collect_sync_target_keys(&concrete_records);
        let sync_target_groups =
            collect_contract_groups(&project.root_dir, &concrete_records, &sync_target_keys);

        let gitignore_sources =
            read_ancestor_gitignore_sources(&gitignore_traversal_root, &directory.dir)?;

        for record in &concrete_records {
            let is_tracked = is_path_tracked_by_git(&gitignore_traversal_root, &record.file.path);
            let is_protected = !is_tracked
                && is_path_protected_by_exact_gitignore(&gitignore_sources, &record.file.path);

            if is_protected {
                continue;
            }

            findings.push(make_finding(FindingInput {
                id: format!("env-gitignore:{}", record.file.path.to_string_lossy()),
                title: format!(
                    "Concrete env file \"{}\" is not protected by .gitignore",
                    record.file.name
                ),
                category: Some("env".to_string()),
                detail: Some(format_gitignore_protection_hint(
                    &project.root_dir,
                    &directory.dir,
                    &record.file.path,
                )),
                file: Some(record.file.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Protect concrete env files with an exact .gitignore entry before committing secrets."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }

        if !sync_target_keys.is_empty() && example_record.is_none() {
            let output_path = directory.dir.join(".env.example");

            findings.push(make_finding(FindingInput {
                id: format!("env-example-missing:{}", directory.dir.to_string_lossy()),
                title: "Missing .env.example contract".to_string(),
                category: Some("env".to_string()),
                detail: Some("Runtime env files exist, but .env.example is missing.".to_string()),
                file: concrete_records
                    .first()
                    .map(|record| record.file.path.clone()),
                fix_ids: vec![format!(
                    "env-example:create:{}",
                    directory.dir.to_string_lossy()
                )],
                fixable: true,
                hint: Some("Run \"maximus fix\" to create a blank contract file.".to_string()),
                severity: Some(Severity::Warn),
            }));

            fixes.push(FixPlan {
                id: format!("env-example:create:{}", directory.dir.to_string_lossy()),
                title: format!(
                    "Create {}",
                    relative_display_path(&project.root_dir, &output_path)
                ),
                files: vec![output_path],
            });
            if options.template_render.source_comments {
                planned_fixes.push(plan_create_env_example_with_groups(
                    &project.root_dir,
                    &directory.dir,
                    sync_target_groups.clone(),
                    options.template_render.clone(),
                ));
            } else {
                planned_fixes.push(plan_create_env_example(
                    &project.root_dir,
                    &directory.dir,
                    &sync_target_keys,
                ));
            }
        }

        if let Some(example_record) = example_record {
            let example_keys = example_record
                .parsed
                .order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let missing_keys = sync_target_keys
                .iter()
                .filter(|key| !example_keys.contains(*key))
                .cloned()
                .collect::<Vec<_>>();

            if !missing_keys.is_empty() {
                findings.push(make_finding(FindingInput {
                    id: format!("env-example-sync:{}", directory.dir.to_string_lossy()),
                    title: format!("{} is missing keys", example_record.file.name),
                    category: Some("env".to_string()),
                    detail: Some(format!("Missing keys: {}.", missing_keys.join(", "))),
                    file: Some(example_record.file.path.clone()),
                    fix_ids: vec![format!(
                        "env-example:sync:{}",
                        directory.dir.to_string_lossy()
                    )],
                    fixable: true,
                    hint: Some(format!(
                        "Run \"maximus fix\" to append the missing keys to {}.",
                        example_record.file.name
                    )),
                    severity: Some(Severity::Warn),
                }));

                fixes.push(FixPlan {
                    id: format!("env-example:sync:{}", directory.dir.to_string_lossy()),
                    title: format!(
                        "Append missing keys to {}",
                        relative_display_path(&project.root_dir, &example_record.file.path)
                    ),
                    files: vec![example_record.file.path.clone()],
                });
                if options.template_render.source_comments {
                    let missing_groups = collect_contract_groups(
                        &project.root_dir,
                        &concrete_records,
                        &missing_keys,
                    );
                    planned_fixes.push(plan_sync_env_example_with_groups(
                        &project.root_dir,
                        &example_record.file.path,
                        &example_record.parsed_source_text,
                        missing_groups,
                        options.template_render.clone(),
                    ));
                } else {
                    planned_fixes.push(plan_sync_env_example(
                        &project.root_dir,
                        &example_record.file.path,
                        &example_record.parsed_source_text,
                        &missing_keys,
                    ));
                }
            }

            for contract_record in &contract_records {
                for entry in &contract_record.parsed.entries {
                    if !looks_like_secret(&entry.key, &entry.value) {
                        continue;
                    }

                    findings.push(make_finding(FindingInput {
                        id: format!(
                            "env-example-secret:{}:{}",
                            contract_record.file.path.to_string_lossy(),
                            entry.key
                        ),
                        title: format!(
                            "{} appears to contain a real value for \"{}\"",
                            contract_record.file.name, entry.key
                        ),
                        category: Some("env".to_string()),
                        detail: Some(
                            "Contract files should describe the interface, not ship concrete secrets."
                                .to_string(),
                        ),
                        file: Some(contract_record.file.path.clone()),
                        fix_ids: Vec::new(),
                        fixable: false,
                        hint: Some(
                            "Replace the value with a blank or placeholder string before sharing the repo."
                                .to_string(),
                        ),
                        severity: Some(Severity::Warn),
                    }));
                }
            }

            if !concrete_records.is_empty() {
                let provided_keys = collect_contract_keys(&concrete_records)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let missing_concrete_keys = example_record
                    .parsed
                    .order
                    .iter()
                    .filter(|key| {
                        !provided_keys.contains(*key)
                            && !missing_concrete_excluded_keys.contains(*key)
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                if !missing_concrete_keys.is_empty() {
                    findings.push(make_finding(FindingInput {
                        id: format!("env-missing-concrete:{}", directory.dir.to_string_lossy()),
                        title: "Declared env contract is not satisfied locally".to_string(),
                        category: Some("env".to_string()),
                        detail: Some(format!(
                            "No concrete value was found for: {}.",
                            missing_concrete_keys.join(", ")
                        )),
                        file: Some(example_record.file.path.clone()),
                        fix_ids: Vec::new(),
                        fixable: false,
                        hint: Some(
                            "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files."
                                .to_string(),
                        ),
                        severity: Some(Severity::Warn),
                    }));
                }
            }
        }

        let base_env = parsed_records
            .iter()
            .find(|record| record.file.name == ".env");
        let local_env = parsed_records
            .iter()
            .find(|record| record.file.name == ".env.local");

        if let (Some(base_env), Some(local_env)) = (base_env, local_env) {
            let mut mismatched_keys = Vec::new();

            for (key, base_entry) in &base_env.parsed.values {
                let Some(local_entry) = local_env.parsed.values.get(key) else {
                    continue;
                };

                if base_entry.value != local_entry.value {
                    mismatched_keys.push(key.clone());
                }
            }

            if !mismatched_keys.is_empty() {
                findings.push(make_finding(FindingInput {
                    id: format!("env-mismatch:{}", directory.dir.to_string_lossy()),
                    title: "Local env overrides detected".to_string(),
                    category: Some("env".to_string()),
                    detail: Some(format!(
                        ".env.local overrides {} key(s): {}.",
                        mismatched_keys.len(),
                        mismatched_keys.join(", ")
                    )),
                    file: Some(local_env.file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Make sure local-only overrides are intentional and documented in .env.example."
                            .to_string(),
                    ),
                    severity: Some(Severity::Info),
                }));
            }
        }
    }

    Ok(CheckOutcome {
        findings: sort_findings(&findings),
        fixes: unique_fixes(&fixes),
        planned_fixes,
    })
}

pub fn render_created_env_example<I, S>(keys: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    render_env_template(keys)
}

pub fn render_synced_env_example(existing_text: &str, missing_keys: &[String]) -> String {
    let prefix = if existing_text.ends_with('\n') || existing_text.is_empty() {
        ""
    } else {
        "\n"
    };
    let addition = render_env_template(missing_keys.iter().map(|key| key.as_str()));

    format!("{existing_text}{prefix}{addition}")
}

pub fn render_created_env_example_with_sources(groups: Vec<EnvTemplateSourceGroup>) -> String {
    render_env_template_groups(
        groups,
        &EnvTemplateRenderOptions {
            source_comments: true,
        },
    )
}

pub fn render_synced_env_example_with_sources(
    existing_text: &str,
    groups: Vec<EnvTemplateSourceGroup>,
) -> String {
    let prefix = if existing_text.ends_with('\n') || existing_text.is_empty() {
        ""
    } else {
        "\n"
    };
    let addition = render_created_env_example_with_sources(groups);

    format!("{existing_text}{prefix}{addition}")
}

#[derive(Debug, Clone)]
struct ParsedEnvRecord {
    file: ProjectFile,
    parsed_source_text: String,
    parsed: maximus_core::ParsedEnv,
}

fn collect_contract_keys(records: &[&ParsedEnvRecord]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut keys = Vec::new();

    for record in records {
        for key in &record.parsed.order {
            if seen.insert(key.clone()) {
                keys.push(key.clone());
            }
        }
    }

    keys
}

fn collect_sync_target_keys(records: &[&ParsedEnvRecord]) -> Vec<String> {
    collect_contract_keys(records)
        .into_iter()
        .filter(|key| !is_ambient_platform_env_key(key))
        .collect()
}

fn is_ambient_platform_env_key(key: &str) -> bool {
    key == "NX_DAEMON"
        || key == "VERCEL"
        || key == "VERCEL_URL"
        || key.starts_with("VERCEL_")
        || key.starts_with("TURBO_")
}

fn collect_contract_groups(
    root_dir: &Path,
    records: &[&ParsedEnvRecord],
    selected_keys: &[String],
) -> Vec<EnvTemplateSourceGroup> {
    let selected = selected_keys.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut groups = Vec::new();

    for record in records {
        let keys = record
            .parsed
            .order
            .iter()
            .filter(|key| selected.contains(*key) && seen.insert((*key).clone()))
            .cloned()
            .collect::<Vec<_>>();

        if keys.is_empty() {
            continue;
        }

        groups.push(EnvTemplateSourceGroup {
            source: Some(relative_display_path(root_dir, &record.file.path)),
            keys,
        });
    }

    groups
}

fn relative_display_path(root_dir: &Path, target: &Path) -> String {
    root_dir
        .strip_prefix(root_dir)
        .ok()
        .and_then(|_| target.strip_prefix(root_dir).ok())
        .map(|path| {
            let rendered = path.to_string_lossy().into_owned();
            if rendered.is_empty() {
                ".".to_string()
            } else {
                rendered
            }
        })
        .unwrap_or_else(|| target.to_string_lossy().into_owned())
}

fn score_contract_record(file_name: &str) -> usize {
    match file_name {
        ".env.example" => 0,
        ".env.sample" => 1,
        ".env.template" => 2,
        ".env.dist" => 3,
        _ => 4,
    }
}

fn find_gitignore_traversal_root(root_dir: &Path) -> PathBuf {
    let mut current = Some(root_dir);

    while let Some(dir) = current {
        if std::fs::symlink_metadata(dir.join(".git")).is_ok() {
            return dir.to_path_buf();
        }

        current = dir.parent();
    }

    root_dir.to_path_buf()
}

fn read_ancestor_gitignore_sources(
    root_dir: &Path,
    directory_dir: &Path,
) -> io::Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    let mut current = root_dir.to_path_buf();

    loop {
        if let Some(text) = read_text_if_exists(&current.join(".gitignore"))? {
            sources.push((current.clone(), text));
        }

        if current == directory_dir {
            break;
        }

        let Ok(relative) = directory_dir.strip_prefix(root_dir) else {
            break;
        };
        let next_depth = current
            .strip_prefix(root_dir)
            .ok()
            .map(|path| path.components().count() + 1)
            .unwrap_or(1);
        let Some(next_component) = relative.components().nth(next_depth - 1) else {
            break;
        };
        current.push(next_component.as_os_str());
    }

    Ok(sources)
}

fn is_path_protected_by_exact_gitignore(
    gitignore_sources: &[(PathBuf, String)],
    target_path: &Path,
) -> bool {
    let mut protected = None;
    let file_name = target_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    for (ignore_root, gitignore_text) in gitignore_sources {
        let relative_path = normalize_path(
            target_path
                .strip_prefix(ignore_root)
                .unwrap_or(target_path)
                .to_string_lossy()
                .as_ref(),
        );

        for pattern in parse_exact_gitignore_patterns(gitignore_text) {
            if pattern_matches_file(&pattern, &relative_path, &file_name) {
                protected = Some(!pattern.negated);
            }
        }
    }

    protected.unwrap_or(false)
}

fn is_path_tracked_by_git(repo_root: &Path, target_path: &Path) -> bool {
    let Ok(relative_path) = target_path.strip_prefix(repo_root) else {
        return false;
    };

    Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg("--")
        .arg(relative_path)
        .output()
        .is_ok_and(|output| output.status.success())
}

struct ExactGitignorePattern {
    pattern: String,
    negated: bool,
    anchored: bool,
    directory_only: bool,
}

fn parse_exact_gitignore_patterns(text: &str) -> Vec<ExactGitignorePattern> {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let negated = line.starts_with('!');
            let raw_pattern = line.strip_prefix('!').unwrap_or(line);
            let Some(pattern) = normalize_gitignore_pattern(raw_pattern) else {
                return None;
            };
            Some(ExactGitignorePattern {
                pattern: pattern.value,
                negated,
                anchored: pattern.anchored,
                directory_only: pattern.directory_only,
            })
        })
        .collect()
}

fn format_gitignore_protection_hint(
    root_dir: &Path,
    directory_dir: &Path,
    file_path: &Path,
) -> String {
    let current_gitignore_display =
        relative_display_path(root_dir, &directory_dir.join(".gitignore"));
    let current_pattern = file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let root_pattern = relative_display_path(root_dir, file_path);

    if directory_dir == root_dir || root_pattern == current_pattern {
        return format!(
            r#"Add "{}" to {}."#,
            current_pattern, current_gitignore_display
        );
    }

    format!(
        r#"Add "{}" to {} or "{}" to .gitignore."#,
        current_pattern, current_gitignore_display, root_pattern
    )
}

struct NormalizedGitignorePattern {
    value: String,
    anchored: bool,
    directory_only: bool,
}

fn normalize_gitignore_pattern(pattern: &str) -> Option<NormalizedGitignorePattern> {
    let normalized = normalize_path(pattern).trim_start_matches("./").to_string();
    let anchored = normalized.starts_with('/');
    let directory_only = normalized.ends_with('/');
    let value = normalized
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();

    if value.is_empty() {
        return None;
    }

    Some(NormalizedGitignorePattern {
        value,
        anchored,
        directory_only,
    })
}

fn pattern_matches_file(
    pattern: &ExactGitignorePattern,
    relative_path: &str,
    file_name: &str,
) -> bool {
    if pattern.directory_only {
        return !pattern.negated && directory_pattern_matches_file(pattern, relative_path);
    }

    directory_pattern_matches_file(pattern, relative_path)
        || gitignore_pattern_matches(&pattern.pattern, relative_path)
        || (!pattern.anchored
            && !pattern.pattern.contains('/')
            && gitignore_pattern_matches(&pattern.pattern, file_name))
}

fn directory_pattern_matches_file(pattern: &ExactGitignorePattern, relative_path: &str) -> bool {
    let Some((directory_path, _)) = relative_path.rsplit_once('/') else {
        return false;
    };
    let directories = directory_ancestors(directory_path);

    if !pattern.anchored && !pattern.pattern.contains('/') {
        return directories.iter().any(|directory| {
            Path::new(directory)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| gitignore_pattern_matches(&pattern.pattern, name))
        });
    }

    directories
        .iter()
        .any(|directory| gitignore_pattern_matches(&pattern.pattern, directory))
}

fn directory_ancestors(directory_path: &str) -> Vec<String> {
    let mut directories = Vec::new();
    let mut current = String::new();

    for segment in directory_path.split('/') {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        directories.push(current.clone());
    }

    directories
}

fn gitignore_pattern_matches(pattern: &str, value: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let value_chars = value.chars().collect::<Vec<_>>();
    let mut memo = vec![vec![None; value_chars.len() + 1]; pattern_chars.len() + 1];

    fn matches(
        pattern_index: usize,
        value_index: usize,
        pattern_chars: &[char],
        value_chars: &[char],
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(result) = memo[pattern_index][value_index] {
            return result;
        }

        let result = if pattern_index >= pattern_chars.len() {
            value_index >= value_chars.len()
        } else if pattern_index + 1 < pattern_chars.len()
            && pattern_chars[pattern_index] == '*'
            && pattern_chars[pattern_index + 1] == '*'
        {
            let after_globstar = pattern_index + 2;
            if after_globstar < pattern_chars.len() && pattern_chars[after_globstar] == '/' {
                let mut matched = matches(
                    after_globstar + 1,
                    value_index,
                    pattern_chars,
                    value_chars,
                    memo,
                );
                let mut index = value_index;
                while !matched && index < value_chars.len() {
                    if value_chars[index] == '/' {
                        matched = matches(
                            after_globstar + 1,
                            index + 1,
                            pattern_chars,
                            value_chars,
                            memo,
                        );
                    }
                    index += 1;
                }
                matched
            } else {
                let mut matched = false;
                let mut index = value_index;
                while !matched && index <= value_chars.len() {
                    matched = matches(after_globstar, index, pattern_chars, value_chars, memo);
                    index += 1;
                }
                matched
            }
        } else if pattern_chars[pattern_index] == '*' {
            let mut matched = matches(
                pattern_index + 1,
                value_index,
                pattern_chars,
                value_chars,
                memo,
            );
            let mut index = value_index;
            while !matched && index < value_chars.len() && value_chars[index] != '/' {
                matched = matches(
                    pattern_index + 1,
                    index + 1,
                    pattern_chars,
                    value_chars,
                    memo,
                );
                index += 1;
            }
            matched
        } else if pattern_chars[pattern_index] == '?' {
            value_index < value_chars.len()
                && value_chars[value_index] != '/'
                && matches(
                    pattern_index + 1,
                    value_index + 1,
                    pattern_chars,
                    value_chars,
                    memo,
                )
        } else {
            value_index < value_chars.len()
                && pattern_chars[pattern_index] == value_chars[value_index]
                && matches(
                    pattern_index + 1,
                    value_index + 1,
                    pattern_chars,
                    value_chars,
                    memo,
                )
        };

        memo[pattern_index][value_index] = Some(result);
        result
    }

    matches(0, 0, &pattern_chars, &value_chars, &mut memo)
}

fn normalize_path(value: &str) -> String {
    value.replace('\\', "/")
}
