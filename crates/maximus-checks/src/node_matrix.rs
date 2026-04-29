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

pub fn run_node_matrix_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    run_node_matrix_check_with_ignore_root(project, &[], &project.root_dir)
}

pub(crate) fn run_node_matrix_check_with_ignore_root(
    project: &ProjectSnapshot,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<CheckOutcome> {
    let workflow =
        collect_workflow_node_versions(&project.root_dir, ignored_patterns, ignore_root)?;
    if workflow.workflow_files == 0 {
        return Ok(CheckOutcome::default());
    }

    let mut findings = Vec::new();
    for package_file in get_files(project, FileKind::Package) {
        if is_ignored_project_path_from_root(ignore_root, &package_file.path, ignored_patterns) {
            continue;
        }
        let Some(engine) = read_node_engine(&package_file.path)? else {
            continue;
        };
        let Some(detail) = node_matrix_detail(&engine, &workflow.node_versions) else {
            continue;
        };

        findings.push(make_finding(FindingInput {
            id: format!("node-matrix:{}", package_file.path.to_string_lossy()),
            title: "Node engine support and GitHub Actions matrix are out of sync".to_string(),
            category: Some("node-matrix".to_string()),
            detail: Some(detail),
            file: Some(package_file.path.clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Update package.json engines.node and the GitHub Actions Node matrix together so CI exercises the supported runtime floor."
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

#[derive(Default)]
struct WorkflowNodeVersions {
    workflow_files: usize,
    node_versions: BTreeSet<u32>,
}

fn collect_workflow_node_versions(
    root_dir: &Path,
    ignored_patterns: &[String],
    ignore_root: &Path,
) -> io::Result<WorkflowNodeVersions> {
    let workflows_dir = root_dir.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(WorkflowNodeVersions::default());
    }
    if is_ignored_project_path_from_root(ignore_root, &workflows_dir, ignored_patterns) {
        return Ok(WorkflowNodeVersions::default());
    }

    let mut workflow_files = 0;
    let mut node_versions = BTreeSet::new();
    let mut entries = fs::read_dir(workflows_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));

    for path in entries {
        if !is_workflow_file(&path) {
            continue;
        }
        if is_ignored_project_path_from_root(ignore_root, &path, ignored_patterns) {
            continue;
        }
        workflow_files += 1;
        let Some(text) = read_text_if_exists(&path)? else {
            continue;
        };
        node_versions.extend(parse_workflow_node_versions(&text));
    }

    Ok(WorkflowNodeVersions {
        workflow_files,
        node_versions,
    })
}

fn read_node_engine(package_path: &Path) -> io::Result<Option<String>> {
    let Some(text) = read_text_if_exists(package_path)? else {
        return Ok(None);
    };
    let Ok(package_json) = parse_jsonc::<Value>(&text, &package_path.to_string_lossy()) else {
        return Ok(None);
    };

    Ok(package_json
        .get("engines")
        .and_then(Value::as_object)
        .and_then(|engines| engines.get("node"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned))
}

fn node_matrix_detail(engine: &str, workflow_versions: &BTreeSet<u32>) -> Option<String> {
    if workflow_versions.is_empty() {
        return Some(format!(
            "package.json engines.node is {engine:?}, but no concrete GitHub Actions Node versions were found."
        ));
    }

    let Some(requirement) = parse_engine_requirement(engine) else {
        return None;
    };
    let mut problems = Vec::new();

    let required_majors = requirement.required_majors();
    let missing = required_majors
        .difference(workflow_versions)
        .copied()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        problems.push(format!(
            "the Actions matrix does not include supported {}",
            render_node_majors(&missing)
        ));
    }

    let unsupported = workflow_versions
        .iter()
        .copied()
        .filter(|version| !requirement.supports(*version))
        .collect::<BTreeSet<_>>();
    if !unsupported.is_empty() {
        problems.push(format!(
            "the Actions matrix includes unsupported {}",
            render_node_majors(&unsupported)
        ));
    }

    if problems.is_empty() {
        return None;
    }

    Some(format!(
        "package.json engines.node is {engine:?}, while GitHub Actions declares {}: {}.",
        render_node_majors(workflow_versions),
        problems.join("; ")
    ))
}

struct EngineRequirement {
    ranges: Vec<EngineRange>,
}

impl EngineRequirement {
    fn supports(&self, major: u32) -> bool {
        self.ranges.iter().any(|range| range.supports(major))
    }

    fn required_majors(&self) -> BTreeSet<u32> {
        self.ranges
            .iter()
            .filter_map(|range| range.min_major)
            .collect()
    }
}

#[derive(Default)]
struct EngineRange {
    min_major: Option<u32>,
    max_major: Option<u32>,
}

impl EngineRange {
    fn supports(&self, major: u32) -> bool {
        if self.min_major.is_some_and(|min_major| major < min_major) {
            return false;
        }
        if self.max_major.is_some_and(|max_major| major > max_major) {
            return false;
        }

        true
    }
}

fn parse_engine_requirement(engine: &str) -> Option<EngineRequirement> {
    let ranges = engine
        .split("||")
        .filter_map(|group| parse_engine_range(group.trim()))
        .collect::<Vec<_>>();

    if ranges.is_empty() {
        None
    } else {
        Some(EngineRequirement { ranges })
    }
}

fn parse_engine_range(group: &str) -> Option<EngineRange> {
    let mut range = EngineRange::default();
    let mut recognized = false;

    for token in engine_range_tokens(group) {
        let token = token.trim_matches(',');
        if let Some(major) = comparator_major(token, ">=") {
            range.min_major = Some(range.min_major.map_or(major, |current| current.max(major)));
            recognized = true;
            continue;
        }
        if let Some(major) = comparator_major(token, ">") {
            let min_major = major.saturating_add(1);
            range.min_major = Some(
                range
                    .min_major
                    .map_or(min_major, |current| current.max(min_major)),
            );
            recognized = true;
            continue;
        }
        if let Some(major) = comparator_major(token, "<=") {
            range.max_major = Some(range.max_major.map_or(major, |current| current.min(major)));
            recognized = true;
            continue;
        }
        if let Some(major) = comparator_major(token, "<") {
            let Some(max_major) = major.checked_sub(1) else {
                continue;
            };
            range.max_major = Some(
                range
                    .max_major
                    .map_or(max_major, |current| current.min(max_major)),
            );
            recognized = true;
            continue;
        }
        if token.starts_with('>') || token.starts_with('<') {
            continue;
        }
        if let Some(major) = major_at_start(
            token
                .trim_start_matches('=')
                .trim_start_matches('^')
                .trim_start_matches('~'),
        ) {
            range.min_major = Some(range.min_major.map_or(major, |current| current.max(major)));
            range.max_major = Some(range.max_major.map_or(major, |current| current.min(major)));
            recognized = true;
        }
    }

    if !recognized
        || range
            .min_major
            .zip(range.max_major)
            .is_some_and(|(min, max)| min > max)
    {
        None
    } else {
        Some(range)
    }
}

fn engine_range_tokens(group: &str) -> Vec<String> {
    let raw_tokens = group
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < raw_tokens.len() {
        let token = raw_tokens[index].trim_matches(',');
        if is_standalone_comparator(token) && index + 1 < raw_tokens.len() {
            tokens.push(format!(
                "{}{}",
                token,
                raw_tokens[index + 1].trim_matches(',')
            ));
            index += 2;
            continue;
        }

        tokens.push(token.to_string());
        index += 1;
    }

    tokens
}

fn is_standalone_comparator(token: &str) -> bool {
    matches!(token, ">=" | ">" | "<=" | "<" | "=")
}

fn comparator_major(token: &str, comparator: &str) -> Option<u32> {
    token
        .strip_prefix(comparator)
        .and_then(|value| major_at_start(value.trim_start_matches('v')))
}

fn major_at_start(value: &str) -> Option<u32> {
    let mut digits = String::new();
    for character in value.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        break;
    }

    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_workflow_node_versions(text: &str) -> BTreeSet<u32> {
    let mut versions = BTreeSet::new();
    let mut active_sequence_indent = None;

    for line in text.lines() {
        let stripped = strip_yaml_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        let indent = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .count();
        if let Some(sequence_indent) = active_sequence_indent {
            if indent > sequence_indent && trimmed.starts_with('-') {
                collect_node_versions(trimmed.trim_start_matches('-').trim(), &mut versions);
                continue;
            }
            if indent <= sequence_indent {
                active_sequence_indent = None;
            }
        }

        let mapping_line = normalize_yaml_sequence_mapping(trimmed);
        let Some((key, value)) = split_yaml_key_value(mapping_line) else {
            continue;
        };
        if key != "node" && key != "node-version" {
            continue;
        }

        if value.is_empty() {
            active_sequence_indent = Some(indent);
        } else {
            collect_node_versions(value, &mut versions);
        }
    }

    versions
}

fn split_yaml_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    Some((key.trim(), value.trim()))
}

fn normalize_yaml_sequence_mapping(line: &str) -> &str {
    line.strip_prefix("- ").map(str::trim_start).unwrap_or(line)
}

fn collect_node_versions(value: &str, versions: &mut BTreeSet<u32>) {
    for token in value.split(|character: char| {
        character == '['
            || character == ']'
            || character == ','
            || character == '"'
            || character == '\''
            || character.is_whitespace()
    }) {
        let token = token.trim_start_matches('v');
        if let Some(version) = major_at_start(token) {
            versions.insert(version);
        }
    }
}

fn strip_yaml_comment(line: &str) -> String {
    let mut output = String::new();
    let mut quote = None;
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            output.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            output.push(character);
            escaped = true;
            continue;
        }
        if quote == Some(character) {
            quote = None;
            output.push(character);
            continue;
        }
        if quote.is_none() && (character == '"' || character == '\'') {
            quote = Some(character);
            output.push(character);
            continue;
        }
        if quote.is_none() && character == '#' {
            break;
        }
        output.push(character);
    }

    output
}

fn is_workflow_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension == "yml" || extension == "yaml")
        .unwrap_or(false)
}

fn render_node_majors(versions: &BTreeSet<u32>) -> String {
    versions
        .iter()
        .map(|version| format!("Node {version}"))
        .collect::<Vec<_>>()
        .join(", ")
}
