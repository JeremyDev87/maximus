use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use maximus_core::{
    is_concrete_env_file_name, is_template_env_file_name, looks_like_secret, make_finding,
    parse_env, plan_create_env_example, plan_sync_env_example, read_text_if_exists,
    render_env_template, sort_findings, unique_fixes, FileKind, FindingInput, FixPlan,
    ProjectFile, ProjectSnapshot, Severity,
};

use crate::check_outcome::CheckOutcome;

pub fn run_env_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();
    let mut fixes = Vec::new();
    let mut planned_fixes = Vec::new();

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
                        "Keep one declaration per env file so overrides stay explicit."
                            .to_string(),
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
        let contract_keys = collect_contract_keys(&concrete_records);

        if !contract_keys.is_empty() && example_record.is_none() {
            let output_path = directory.dir.join(".env.example");

            findings.push(make_finding(FindingInput {
                id: format!("env-example-missing:{}", directory.dir.to_string_lossy()),
                title: "Missing .env.example contract".to_string(),
                category: Some("env".to_string()),
                detail: Some("Runtime env files exist, but .env.example is missing.".to_string()),
                file: concrete_records.first().map(|record| record.file.path.clone()),
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
            planned_fixes.push(plan_create_env_example(
                &project.root_dir,
                &directory.dir,
                &contract_keys,
            ));
        }

        if let Some(example_record) = example_record {
            let example_keys = example_record
                .parsed
                .order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let missing_keys = contract_keys
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
                planned_fixes.push(plan_sync_env_example(
                    &project.root_dir,
                    &example_record.file.path,
                    &example_record.parsed_source_text,
                    &missing_keys,
                ));
            }

            for contract_record in &contract_records {
                for entry in &contract_record.parsed.entries {
                    if !looks_like_secret(&entry.value) {
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
                    .filter(|key| !provided_keys.contains(*key))
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

        let base_env = parsed_records.iter().find(|record| record.file.name == ".env");
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
