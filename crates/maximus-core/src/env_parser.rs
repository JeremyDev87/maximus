use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};

use crate::text_order::{compare_env_template_keys, locale_compare_like};
use indexmap::IndexMap;

pub use crate::text_order::EnvTemplateSortMode;

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
    pub include_source_comments: bool,
    pub sort_mode: EnvTemplateSortMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvTemplateSourceGroup {
    pub source: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvTemplateRenderContext {
    pub source_groups: Vec<EnvTemplateSourceGroup>,
}

thread_local! {
    static ENV_TEMPLATE_RENDER_OPTIONS: RefCell<EnvTemplateRenderOptions> =
        RefCell::new(EnvTemplateRenderOptions::default());
    static ENV_TEMPLATE_RENDER_CONTEXTS: RefCell<VecDeque<EnvTemplateRenderContext>> =
        RefCell::new(VecDeque::new());
}

pub fn env_template_render_options() -> EnvTemplateRenderOptions {
    ENV_TEMPLATE_RENDER_OPTIONS.with(|options| options.borrow().clone())
}

pub fn set_env_template_render_options(options: EnvTemplateRenderOptions) {
    ENV_TEMPLATE_RENDER_OPTIONS.with(|current| {
        *current.borrow_mut() = options;
    });
}

pub fn register_env_template_render_context(context: EnvTemplateRenderContext) {
    ENV_TEMPLATE_RENDER_CONTEXTS.with(|contexts| {
        contexts.borrow_mut().push_back(context);
    });
}

pub fn clear_env_template_render_contexts() {
    ENV_TEMPLATE_RENDER_CONTEXTS.with(|contexts| {
        contexts.borrow_mut().clear();
    });
}

pub fn reset_env_template_render_state() {
    ENV_TEMPLATE_RENDER_OPTIONS.with(|current| {
        *current.borrow_mut() = EnvTemplateRenderOptions::default();
    });
    clear_env_template_render_contexts();
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
    let unique_keys = keys
        .into_iter()
        .map(|key| key.as_ref().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if unique_keys.is_empty() {
        return String::new();
    }

    let options = env_template_render_options();

    if options.include_source_comments {
        if let Some(rendered) =
            render_env_template_with_source_comments(&unique_keys, options.sort_mode)
        {
            return rendered;
        }
    }

    render_env_template_lines(&unique_keys, options.sort_mode)
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

fn render_env_template_with_source_comments(
    unique_keys: &[String],
    sort_mode: EnvTemplateSortMode,
) -> Option<String> {
    let context =
        ENV_TEMPLATE_RENDER_CONTEXTS.with(|contexts| contexts.borrow_mut().pop_front())?;
    let requested_keys = unique_keys.iter().cloned().collect::<BTreeSet<_>>();
    let mut groups = context
        .source_groups
        .into_iter()
        .filter_map(|group| {
            let keys = group
                .keys
                .into_iter()
                .filter(|key| requested_keys.contains(key))
                .collect::<Vec<_>>();

            if keys.is_empty() {
                None
            } else {
                Some(EnvTemplateSourceGroup {
                    source: group.source,
                    keys,
                })
            }
        })
        .collect::<Vec<_>>();

    if groups.is_empty() {
        return None;
    }

    groups.sort_by(|left, right| locale_compare_like(&left.source, &right.source));

    let mut lines = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }

        lines.push(format!("# source: {}", group.source));
        lines.extend(render_keys_for_group(group.keys, sort_mode));
    }

    Some(render_lines(lines))
}

fn render_env_template_lines(keys: &[String], sort_mode: EnvTemplateSortMode) -> String {
    let mut rendered_keys = keys.to_vec();
    rendered_keys.sort_by(|left, right| compare_env_template_keys(left, right, sort_mode));

    render_lines(
        rendered_keys
            .into_iter()
            .map(|key| format!("{key}="))
            .collect(),
    )
}

fn render_keys_for_group(mut keys: Vec<String>, sort_mode: EnvTemplateSortMode) -> Vec<String> {
    keys.sort_by(|left, right| compare_env_template_keys(left, right, sort_mode));
    keys.dedup();
    keys.into_iter().map(|key| format!("{key}=")).collect()
}

fn render_lines(lines: Vec<String>) -> String {
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}
