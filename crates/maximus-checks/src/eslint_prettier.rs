use maximus_core::{make_finding, FileKind, FindingInput, ProjectSnapshot, Severity, read_text_if_exists};

use crate::check_outcome::CheckOutcome;
use crate::registry::{package_file_for_directory, read_package_json};

const FORMATTING_RULES: &[&str] = &[
    "array-bracket-spacing",
    "comma-dangle",
    "indent",
    "max-len",
    "object-curly-spacing",
    "quotes",
    "semi",
];

pub fn run_eslint_prettier_check(project: &ProjectSnapshot) -> std::io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let eslint_files = directory
            .files_by_kind
            .get(&FileKind::Eslint)
            .cloned()
            .unwrap_or_default();
        let prettier_files = directory
            .files_by_kind
            .get(&FileKind::Prettier)
            .cloned()
            .unwrap_or_default();
        let package_file = package_file_for_directory(directory).cloned();
        let package_json = package_file
            .as_ref()
            .and_then(|file| read_package_json(&file.path));

        let mut eslint_sources = Vec::new();
        let mut prettier_sources = Vec::new();

        for file in &eslint_files {
            if let Some(text) = read_text_if_exists(&file.path)? {
                eslint_sources.push(text);
            }
        }

        for file in &prettier_files {
            if let Some(text) = read_text_if_exists(&file.path)? {
                prettier_sources.push(text);
            }
        }

        if let Some(package_json) = package_json.as_ref() {
            if let Some(value) = package_json
                .get("eslintConfig")
                .filter(|value| is_js_truthy_json_value(value))
            {
                eslint_sources.push(
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
                );
            }

            if let Some(value) = package_json
                .get("prettier")
                .filter(|value| is_js_truthy_json_value(value))
            {
                prettier_sources.push(
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
                );
            }
        }

        if eslint_sources.is_empty() || prettier_sources.is_empty() {
            continue;
        }

        let eslint_text = eslint_sources.join("\n");
        let has_formatting_rules = FORMATTING_RULES
            .iter()
            .any(|rule| contains_property_name(&eslint_text, rule));
        let has_prettier_integration = contains_word(&eslint_text, "prettier");
        let file = eslint_files
            .first()
            .map(|file| file.path.clone())
            .or_else(|| package_file.as_ref().map(|file| file.path.clone()));

        if has_formatting_rules && !has_prettier_integration {
            findings.push(make_finding(FindingInput {
                id: format!("eslint-prettier-conflict:{}", directory.dir.to_string_lossy()),
                title: "ESLint formatting rules may conflict with Prettier".to_string(),
                category: Some("conflict".to_string()),
                detail: Some(
                    "Formatting-oriented ESLint rules were found, but no explicit Prettier bridge was detected."
                        .to_string(),
                ),
                file,
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Consider eslint-config-prettier or plugin:prettier/recommended to reduce formatter churn."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        } else if !has_prettier_integration {
            findings.push(make_finding(FindingInput {
                id: format!("eslint-prettier-separate:{}", directory.dir.to_string_lossy()),
                title: "ESLint and Prettier are configured separately".to_string(),
                category: Some("conflict".to_string()),
                detail: Some(
                    "That can be fine, but teams often prefer an explicit integration strategy."
                        .to_string(),
                ),
                file,
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Document which tool owns formatting and which tool owns code-quality rules."
                        .to_string(),
                ),
                severity: Some(Severity::Info),
            }));
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn contains_property_name(text: &str, property: &str) -> bool {
    ['"', '\''].into_iter().any(|quote| {
        let needle = format!("{quote}{property}{quote}");
        let mut remainder = text;

        while let Some(index) = remainder.find(&needle) {
            let after_match = &remainder[index + needle.len()..];
            let next_non_whitespace = after_match.chars().find(|character| !character.is_whitespace());

            if next_non_whitespace == Some(':') {
                return true;
            }

            remainder = after_match;
        }

        false
    })
}

fn contains_word(text: &str, needle: &str) -> bool {
    let mut search_start = 0usize;

    while let Some(relative_index) = text[search_start..].find(needle) {
        let start = search_start + relative_index;
        let end = start + needle.len();
        let previous = text[..start].chars().next_back();
        let next = text[end..].chars().next();

        if !is_word_char(previous) && !is_word_char(next) {
            return true;
        }

        search_start = end;
    }

    false
}

fn is_word_char(character: Option<char>) -> bool {
    character
        .map(|character| character.is_alphanumeric() || character == '_')
        .unwrap_or(false)
}

fn is_js_truthy_json_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => {
            value.as_i64().map(|value| value != 0).unwrap_or_else(|| {
                value
                    .as_u64()
                    .map(|value| value != 0)
                    .unwrap_or_else(|| value.as_f64().map(|value| value != 0.0).unwrap_or(true))
            })
        }
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}
