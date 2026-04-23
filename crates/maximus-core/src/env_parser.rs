use std::collections::BTreeSet;

use indexmap::IndexMap;
use crate::text_order::locale_compare_like;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvEntry {
    pub key: String,
    pub raw_value: String,
    pub value: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvDuplicate {
    pub key: String,
    pub first_line: usize,
    pub second_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidEnvLine {
    pub label: String,
    pub line: usize,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEnv {
    pub entries: Vec<EnvEntry>,
    pub duplicates: Vec<EnvDuplicate>,
    pub invalid_lines: Vec<InvalidEnvLine>,
    pub order: Vec<String>,
    pub values: IndexMap<String, EnvEntry>,
}

pub fn parse_env(text: &str, label: Option<&str>) -> ParsedEnv {
    let label = label.unwrap_or(".env").to_string();
    let mut entries = Vec::new();
    let mut duplicates = Vec::new();
    let mut invalid_lines = Vec::new();
    let mut values: IndexMap<String, EnvEntry> = IndexMap::new();
    let mut order = Vec::new();

    for (index, line) in text.split('\n').enumerate() {
        let raw_line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = trim_js_like(raw_line);

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let line = trimmed
            .strip_prefix("export ")
            .map(trim_js_like)
            .unwrap_or(trimmed);

        let Some((key, raw_value)) = split_env_assignment(line) else {
            invalid_lines.push(InvalidEnvLine {
                label: label.clone(),
                line: index + 1,
                content: raw_line.to_string(),
            });
            continue;
        };

        if !is_valid_env_key(key) {
            invalid_lines.push(InvalidEnvLine {
                label: label.clone(),
                line: index + 1,
                content: raw_line.to_string(),
            });
            continue;
        }

        let entry = EnvEntry {
            key: key.to_string(),
            raw_value: raw_value.to_string(),
            value: normalize_env_value(raw_value),
            line: index + 1,
        };

        if let Some(previous_entry) = values.get(key) {
            duplicates.push(EnvDuplicate {
                key: key.to_string(),
                first_line: previous_entry.line,
                second_line: index + 1,
            });
        } else {
            order.push(key.to_string());
        }

        values.insert(key.to_string(), entry.clone());
        entries.push(entry);
    }

    ParsedEnv {
        entries,
        duplicates,
        invalid_lines,
        order,
        values,
    }
}

pub fn render_env_template<I, S>(keys: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut unique_keys = keys
        .into_iter()
        .map(|key| key.as_ref().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if unique_keys.is_empty() {
        return String::new();
    }

    unique_keys.sort_by(|left, right| locale_compare_like(left, right));

    let mut rendered = unique_keys
        .into_iter()
        .map(|key| format!("{key}="))
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

pub fn is_template_env_file_name(name: &str) -> bool {
    if !is_env_file_name(name) {
        return false;
    }

    let segments = name
        .split('.')
        .filter(|segment| !segment.is_empty())
        .skip(1);

    segments
        .map(|segment| segment.to_ascii_lowercase())
        .any(|segment| matches!(segment.as_str(), "dist" | "example" | "sample" | "template"))
}

pub fn is_concrete_env_file_name(name: &str) -> bool {
    is_env_file_name(name) && !is_template_env_file_name(name)
}

pub fn looks_like_secret(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    if is_placeholder_value(value) {
        return false;
    }

    value.len() >= 16
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/_+=-".contains(character))
}

fn split_env_assignment(line: &str) -> Option<(&str, &str)> {
    let equals_index = line.find('=')?;
    let key = trim_js_like(line[..equals_index].trim());
    let raw_value = trim_js_like_start(&line[equals_index + 1..]);
    Some((key, raw_value))
}

fn is_env_file_name(name: &str) -> bool {
    name == ".env"
        || name
            .strip_prefix(".env.")
            .map(|suffix| !suffix.is_empty())
            .unwrap_or(false)
}

fn is_valid_env_key(key: &str) -> bool {
    let mut characters = key.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-'))
}

fn trim_js_like(value: &str) -> &str {
    value.trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}')
}

fn trim_js_like_start(value: &str) -> &str {
    value.trim_start_matches(|character: char| {
        character.is_whitespace() || character == '\u{feff}'
    })
}

fn normalize_env_value(raw_value: &str) -> String {
    let trimmed = raw_value.trim();

    if trimmed.len() >= 2 {
        let first = trimmed.chars().next().unwrap();
        let last = trimmed.chars().last().unwrap();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }

    trimmed.to_string()
}

fn is_placeholder_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();

    matches!(
        lower.as_str(),
        "change-me"
            | "example"
            | "placeholder"
            | "localhost"
            | "127.0.0.1"
            | "true"
            | "false"
            | "0"
            | "1"
    ) || lower
        .strip_prefix("your-")
        .map(|tail| {
            !tail.is_empty()
                && tail
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '-')
        })
        .unwrap_or(false)
}
