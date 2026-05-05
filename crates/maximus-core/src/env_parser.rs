use std::collections::BTreeSet;
use std::fmt::Write;

use crate::text_order::locale_compare_like;
use indexmap::IndexMap;

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvTemplateRenderOptions {
    pub source_comments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvTemplateSourceGroup {
    pub source: Option<String>,
    pub keys: Vec<String>,
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
    render_env_template_groups(
        [EnvTemplateSourceGroup {
            source: None,
            keys: keys
                .into_iter()
                .map(|key| key.as_ref().to_string())
                .collect(),
        }],
        &EnvTemplateRenderOptions::default(),
    )
}

pub fn render_env_template_groups<I>(groups: I, options: &EnvTemplateRenderOptions) -> String
where
    I: IntoIterator<Item = EnvTemplateSourceGroup>,
{
    let mut groups = groups
        .into_iter()
        .map(normalize_env_template_source_group)
        .filter(|group| !group.keys.is_empty())
        .collect::<Vec<_>>();

    if groups.is_empty() {
        return String::new();
    }

    if options.source_comments {
        groups.sort_by(compare_source_groups);
    }

    let mut rendered = String::new();
    for (index, group) in groups.iter().enumerate() {
        if index > 0 && options.source_comments {
            rendered.push('\n');
        }

        if options.source_comments {
            if let Some(source) = &group.source {
                let _ = writeln!(rendered, "# Source: {source}");
            }
        }

        for key in &group.keys {
            let _ = writeln!(rendered, "{key}=");
        }
    }

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

pub fn looks_like_secret(key: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    if is_placeholder_value(value) {
        return false;
    }

    if has_high_confidence_secret_value(value) {
        return true;
    }

    is_secret_like_env_key(key)
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
    value.trim_start_matches(|character: char| character.is_whitespace() || character == '\u{feff}')
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

fn has_high_confidence_secret_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();

    value.contains("-----BEGIN PRIVATE KEY-----")
        || lower.starts_with("sk_live_")
        || lower.starts_with("sk_test_")
        || lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("xoxp-")
        || lower.starts_with("xoxa-")
        || (value.len() == 20
            && value.starts_with("AKIA")
            && value
                .chars()
                .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit()))
        || (value.len() >= 35
            && value.starts_with("AIza")
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character)))
}

fn is_secret_like_env_key(key: &str) -> bool {
    let segments = env_key_segments(key);
    if segments.is_empty() {
        return false;
    }

    if contains_adjacent_segments(&segments, "PRIVATE", "KEY")
        || contains_service_key_segments(&segments)
    {
        return true;
    }

    if segments.iter().any(|segment| {
        matches!(
            segment.as_str(),
            "TOKEN" | "SECRET" | "PASSWORD" | "PASSWD" | "PWD"
        )
    }) {
        return true;
    }

    if contains_adjacent_segments(&segments, "API", "KEY")
        || contains_adjacent_segments(&segments, "ACCESS", "KEY")
    {
        return !is_public_key_identifier(&segments);
    }

    false
}

fn env_key_segments(key: &str) -> Vec<String> {
    key.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_ascii_uppercase())
        .collect()
}

fn contains_adjacent_segments(segments: &[String], left: &str, right: &str) -> bool {
    segments
        .windows(2)
        .any(|window| window[0] == left && window[1] == right)
}

fn contains_service_key_segments(segments: &[String]) -> bool {
    segments
        .windows(2)
        .any(|window| window[0] == "SERVICE" && window[1] == "KEY")
        || segments.windows(3).any(|window| {
            window[0] == "SERVICE"
                && matches!(window[1].as_str(), "ROLE" | "ACCOUNT")
                && window[2] == "KEY"
        })
}

fn is_public_key_identifier(segments: &[String]) -> bool {
    contains_adjacent_segments(segments, "PUBLIC", "KEY")
        || contains_adjacent_segments(segments, "ANON", "KEY")
        || contains_adjacent_segments(segments, "CLIENT", "ID")
}

fn normalize_env_template_source_group(group: EnvTemplateSourceGroup) -> EnvTemplateSourceGroup {
    let mut unique_keys = group
        .keys
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unique_keys.sort_by(|left, right| locale_compare_like(left, right));

    EnvTemplateSourceGroup {
        source: group.source,
        keys: unique_keys,
    }
}

fn compare_source_groups(
    left: &EnvTemplateSourceGroup,
    right: &EnvTemplateSourceGroup,
) -> std::cmp::Ordering {
    match (&left.source, &right.source) {
        (Some(left), Some(right)) => locale_compare_like(left, right),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
