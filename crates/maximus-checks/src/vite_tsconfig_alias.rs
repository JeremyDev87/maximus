use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{
    make_finding, parse_jsonc, read_text_if_exists, FindingInput, ProjectSnapshot, Severity,
};
use serde_json::{Map, Value};

use crate::check_outcome::CheckOutcome;
use crate::registry::tsconfig_entry_file_for_directory;

pub fn run_vite_tsconfig_alias_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let Some(vite_file) = vite_config_file_for_directory(directory) else {
            continue;
        };
        let tsconfig_file = tsconfig_entry_file_for_directory(directory);

        let Some(vite_text) = read_text_if_exists(&vite_file.path)? else {
            continue;
        };
        let Some(vite_aliases) = parse_vite_aliases(&vite_text, &vite_file.path) else {
            continue;
        };

        let tsconfig_aliases = if let Some(tsconfig_file) = tsconfig_file {
            let Some(tsconfig_aliases) = read_effective_tsconfig_aliases(&tsconfig_file.path)?
            else {
                continue;
            };
            tsconfig_aliases
        } else {
            BTreeMap::new()
        };

        let mut alias_keys = BTreeSet::new();
        alias_keys.extend(vite_aliases.keys().cloned());
        alias_keys.extend(tsconfig_aliases.keys().cloned());

        for alias_key in alias_keys {
            match (
                vite_aliases.get(&alias_key),
                tsconfig_aliases.get(&alias_key),
            ) {
                (Some(vite_target), Some(tsconfig_target)) if vite_target == tsconfig_target => {}
                (Some(vite_target), Some(tsconfig_target)) => findings.push(make_finding(FindingInput {
                    id: format!(
                        "vite-alias-sync:{}:{}",
                        vite_file.path.to_string_lossy(),
                        alias_key
                    ),
                    title: format!("Vite alias \"{alias_key}\" differs from tsconfig paths"),
                    category: Some("vite-tsconfig-alias".to_string()),
                    detail: Some(format!(
                        "Vite resolves \"{alias_key}\" to {vite_target}, but tsconfig paths resolves it to {tsconfig_target}."
                    )),
                    file: Some(vite_file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Align both alias surfaces so editor and bundler resolution stay in sync."
                            .to_string(),
                    ),
                    severity: Some(Severity::Warn),
                })),
                (Some(vite_target), None) => findings.push(make_finding(FindingInput {
                    id: format!(
                        "vite-alias-sync:{}:{}:vite-only",
                        vite_file.path.to_string_lossy(),
                        alias_key
                    ),
                    title: format!("Vite alias \"{alias_key}\" is missing from tsconfig paths"),
                    category: Some("vite-tsconfig-alias".to_string()),
                    detail: Some(format!(
                        "Vite resolves \"{alias_key}\" to {vite_target}, but tsconfig paths does not declare the same key."
                    )),
                    file: Some(vite_file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Add the alias to tsconfig paths or remove it from Vite so imports resolve consistently."
                            .to_string(),
                    ),
                    severity: Some(Severity::Info),
                })),
                (None, Some(tsconfig_target)) => findings.push(make_finding(FindingInput {
                    id: format!(
                        "vite-alias-sync:{}:{}:tsconfig-only",
                        tsconfig_file
                            .as_ref()
                            .map(|file| file.path.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        alias_key
                    ),
                    title: format!(
                        "tsconfig path alias \"{alias_key}\" is missing from Vite"
                    ),
                    category: Some("vite-tsconfig-alias".to_string()),
                    detail: Some(format!(
                        "tsconfig paths resolves \"{alias_key}\" to {tsconfig_target}, but Vite does not declare the same key."
                    )),
                    file: tsconfig_file.map(|file| file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Mirror the alias in Vite config or remove it from tsconfig paths if it is not meant for runtime resolution."
                            .to_string(),
                    ),
                    severity: Some(Severity::Info),
                })),
                (None, None) => {}
            }
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn vite_config_file_for_directory(
    directory: &maximus_core::ProjectDirectory,
) -> Option<&maximus_core::ProjectFile> {
    directory
        .files_by_kind
        .get(&maximus_core::FileKind::Vite)
        .and_then(|files| {
            files
                .iter()
                .find(|file| file.name.starts_with("vite.config."))
        })
}

fn parse_vite_aliases(text: &str, config_path: &Path) -> Option<BTreeMap<String, String>> {
    let config_object_start = find_vite_config_object_start(text)?;
    let Some(resolve_value_start) =
        find_object_property_value_start(text, config_object_start, "resolve")?
    else {
        return Some(BTreeMap::new());
    };
    let resolve_value_start = skip_ws_and_comments(text, resolve_value_start);
    let Some('{') = text[resolve_value_start..].chars().next() else {
        return None;
    };

    let Some(value_start) = find_object_property_value_start(text, resolve_value_start, "alias")?
    else {
        return Some(BTreeMap::new());
    };
    let value_start = skip_ws_and_comments(text, value_start);
    match text[value_start..].chars().next()? {
        '{' => parse_string_map_from_object(text, value_start, config_path),
        '[' => parse_alias_array(text, value_start, config_path),
        _ => None,
    }
}

fn find_vite_config_object_start(text: &str) -> Option<usize> {
    let mut cursor = find_export_default_value_start(text)?;

    if text[cursor..].starts_with("defineConfig") {
        cursor += "defineConfig".len();
        cursor = skip_ws_and_comments(text, cursor);
        let Some('(') = text[cursor..].chars().next() else {
            return None;
        };
        cursor = skip_ws_and_comments(text, cursor + 1);
    }

    match text[cursor..].chars().next()? {
        '{' => Some(cursor),
        _ => None,
    }
}

fn find_export_default_value_start(text: &str) -> Option<usize> {
    let mut index = 0;

    while index < text.len() {
        let ch = text[index..].chars().next()?;

        match ch {
            '\'' | '"' => {
                let (_, next_index) = parse_string_literal(text, index)?;
                index = next_index;
                continue;
            }
            '`' => {
                index = skip_template_literal(text, index)?;
                continue;
            }
            '/' if text[index + ch.len_utf8()..].starts_with('/') => {
                index += 2;
                while index < text.len() {
                    let next = text[index..].chars().next()?;
                    index += next.len_utf8();
                    if next == '\n' {
                        break;
                    }
                }
                continue;
            }
            '/' if text[index + ch.len_utf8()..].starts_with('*') => {
                index += 2;
                while index < text.len() {
                    let next = text[index..].chars().next()?;
                    index += next.len_utf8();
                    if next == '*' && text[index..].starts_with('/') {
                        index += 1;
                        break;
                    }
                }
                continue;
            }
            _ => {}
        }

        if starts_with_keyword(text, index, "export") {
            let cursor = skip_ws_and_comments(text, index + "export".len());
            if starts_with_keyword(text, cursor, "default") {
                return Some(skip_ws_and_comments(text, cursor + "default".len()));
            }
        }

        index += ch.len_utf8();
    }

    None
}

fn starts_with_keyword(text: &str, index: usize, keyword: &str) -> bool {
    if index + keyword.len() > text.len() || &text[index..index + keyword.len()] != keyword {
        return false;
    }
    if index > 0 && is_identifier_char(text.as_bytes()[index - 1] as char) {
        return false;
    }
    let next_index = index + keyword.len();
    if next_index < text.len() {
        if let Some(next) = text[next_index..].chars().next() {
            if is_identifier_char(next) {
                return false;
            }
        }
    }

    true
}

fn find_object_property_value_start(
    text: &str,
    object_start: usize,
    property: &str,
) -> Option<Option<usize>> {
    let object_end = find_matching_delimiter(text, object_start, '{', '}')?;
    let mut cursor = object_start + 1;

    while cursor < object_end {
        cursor = skip_ws_and_comments(text, cursor);
        if cursor >= object_end || text[cursor..].starts_with('}') {
            break;
        }
        if text[cursor..].starts_with(',') {
            cursor += 1;
            continue;
        }

        let (key, next_cursor) = parse_string_or_identifier(text, cursor)?;
        let mut value_cursor = skip_ws_and_comments(text, next_cursor);
        let Some(':') = text[value_cursor..].chars().next() else {
            return None;
        };
        value_cursor = skip_ws_and_comments(text, value_cursor + 1);
        if key == property {
            return Some(Some(value_cursor));
        }

        cursor = skip_value(text, value_cursor, object_end)?;
    }

    Some(None)
}

fn parse_alias_array(
    text: &str,
    array_start: usize,
    config_path: &Path,
) -> Option<BTreeMap<String, String>> {
    let array_end = find_matching_delimiter(text, array_start, '[', ']')?;
    let mut aliases = BTreeMap::new();
    let mut cursor = array_start + 1;

    while cursor < array_end {
        cursor = skip_ws_and_comments(text, cursor);
        if cursor >= array_end {
            break;
        }
        if text[cursor..].starts_with(']') {
            break;
        }
        if text[cursor..].starts_with(',') {
            cursor += 1;
            continue;
        }

        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if ch != '{' {
            cursor = skip_value(text, cursor, array_end)?;
            continue;
        }

        let entry_end = find_matching_delimiter(text, cursor, '{', '}')?;
        if let Some((alias_key, alias_target)) =
            parse_alias_array_entry(text, cursor + 1, entry_end, config_path)
        {
            aliases.insert(normalize_alias_key(&alias_key), alias_target);
        }
        cursor = entry_end + 1;
    }

    Some(aliases)
}

fn parse_alias_array_entry(
    text: &str,
    mut cursor: usize,
    entry_end: usize,
    config_path: &Path,
) -> Option<(String, String)> {
    let mut find_key = None;
    let mut replacement = None;

    while cursor < entry_end {
        cursor = skip_ws_and_comments(text, cursor);
        if cursor >= entry_end {
            break;
        }
        if text[cursor..].starts_with('}') {
            break;
        }
        if text[cursor..].starts_with(',') {
            cursor += 1;
            continue;
        }

        let (key, next_cursor) = parse_string_or_identifier(text, cursor)?;
        let mut value_cursor = skip_ws_and_comments(text, next_cursor);
        let Some(':') = text[value_cursor..].chars().next() else {
            return None;
        };
        value_cursor = skip_ws_and_comments(text, value_cursor + 1);

        match key.as_str() {
            "find" => {
                let (value, next_cursor) = parse_string_literal(text, value_cursor)?;
                find_key = Some(value);
                cursor = next_cursor;
            }
            "replacement" => {
                let (value, next_cursor) = parse_alias_value(text, value_cursor)?;
                replacement = Some(value);
                cursor = next_cursor;
            }
            _ => {
                cursor = skip_value(text, value_cursor, entry_end)?;
            }
        }
    }

    let find_key = find_key?;
    let replacement = replacement?;
    let config_dir = config_path.parent().unwrap_or(config_path);
    let normalized = normalize_comparable_alias_target(config_dir, &replacement)?;

    Some((find_key, normalized))
}

fn skip_value(text: &str, cursor: usize, entry_end: usize) -> Option<usize> {
    let Some(ch) = text[cursor..].chars().next() else {
        return None;
    };
    match ch {
        '{' => find_matching_delimiter(text, cursor, '{', '}').map(|end| end + 1),
        '[' => find_matching_delimiter(text, cursor, '[', ']').map(|end| end + 1),
        '(' => find_matching_delimiter(text, cursor, '(', ')').map(|end| end + 1),
        '\'' | '"' => parse_string_literal(text, cursor).map(|(_, end)| end),
        '`' => skip_template_literal(text, cursor),
        _ => {
            let mut next = cursor;
            while next < entry_end {
                let Some(current) = text[next..].chars().next() else {
                    break;
                };
                if current == ',' || current == '}' {
                    break;
                }
                next += current.len_utf8();
            }
            if next == cursor {
                Some(cursor + ch.len_utf8())
            } else {
                Some(next)
            }
        }
    }
}

fn read_effective_tsconfig_aliases(
    config_path: &Path,
) -> io::Result<Option<BTreeMap<String, String>>> {
    let mut visited = HashSet::new();
    read_effective_tsconfig_aliases_inner(config_path, &mut visited)
}

fn read_effective_tsconfig_aliases_inner(
    config_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<Option<BTreeMap<String, String>>> {
    let normalized_path = normalize_tsconfig_path(config_path);
    if !visited.insert(normalized_path) {
        return Ok(None);
    }

    let Some(config) = read_tsconfig_json(config_path)? else {
        return Ok(None);
    };

    let mut aliases = if let Some(parent_config_path) =
        load_extended_tsconfig_path(config_path, &config, visited)?
    {
        read_effective_tsconfig_aliases_inner(&parent_config_path, visited)?.unwrap_or_default()
    } else {
        BTreeMap::new()
    };

    if let Some(config_options) = config.get("compilerOptions").and_then(Value::as_object) {
        if config_options.contains_key("paths") {
            aliases = parse_tsconfig_aliases(config_options, config_path);
        }
    }

    Ok(Some(aliases))
}

fn parse_tsconfig_aliases(
    compiler_options: &Map<String, Value>,
    config_path: &Path,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    let Some(paths) = compiler_options.get("paths").and_then(Value::as_object) else {
        return aliases;
    };

    let config_dir = config_path.parent().unwrap_or(config_path);
    let base_dir = compiler_options
        .get("baseUrl")
        .and_then(Value::as_str)
        .and_then(|base_url| normalize_base_url(config_dir, base_url))
        .unwrap_or_else(|| config_dir.to_path_buf());

    for (alias_key, targets) in paths {
        let Some(first_target) = targets
            .as_array()
            .and_then(|entries| entries.iter().find_map(Value::as_str))
        else {
            continue;
        };

        if let Some(normalized) = normalize_comparable_alias_target(&base_dir, first_target) {
            aliases.insert(normalize_alias_key(alias_key), normalized);
        }
    }

    aliases
}

fn normalize_base_url(config_dir: &Path, base_url: &str) -> Option<PathBuf> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() || has_url_like_prefix(trimmed) {
        return None;
    }

    let base_url = trimmed.replace('\\', "/");
    let base_path = Path::new(&base_url);
    if base_path.is_absolute() {
        Some(PathBuf::from(base_path))
    } else {
        Some(config_dir.join(base_path))
    }
}

fn read_tsconfig_json(file_path: &Path) -> io::Result<Option<Value>> {
    let Some(text) = read_text_if_exists(file_path)? else {
        return Ok(None);
    };

    Ok(parse_jsonc::<Value>(&text, &file_path.to_string_lossy()).ok())
}

fn load_extended_tsconfig_path(
    config_path: &Path,
    config: &Value,
    visited: &HashSet<PathBuf>,
) -> io::Result<Option<PathBuf>> {
    let Some(extends_path) = config.get("extends").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(parent_config_path) = resolve_extends_config_path(
        config_path.parent().unwrap_or_else(|| Path::new(".")),
        extends_path,
    ) else {
        return Ok(None);
    };

    if visited.contains(&normalize_tsconfig_path(&parent_config_path)) {
        return Ok(None);
    }

    Ok(Some(parent_config_path))
}

fn resolve_extends_config_path(base_dir: &Path, extends_path: &str) -> Option<PathBuf> {
    let extends_candidate = Path::new(extends_path);
    let is_local_extends = extends_candidate.is_absolute()
        || extends_path.starts_with("./")
        || extends_path.starts_with("../")
        || extends_path.starts_with(".\\")
        || extends_path.starts_with("..\\");

    if is_local_extends {
        return resolve_tsconfig_candidate(&base_dir.join(extends_path.replace('\\', "/")))
            .ok()
            .flatten();
    }

    for ancestor in base_dir.ancestors() {
        let candidate = ancestor.join("node_modules").join(extends_path);
        if let Ok(Some(resolved)) = resolve_tsconfig_candidate(&candidate) {
            return Some(resolved);
        }
    }

    None
}

fn resolve_tsconfig_candidate(candidate: &Path) -> io::Result<Option<PathBuf>> {
    if candidate.exists() {
        let metadata = fs::metadata(candidate)?;
        if metadata.is_dir() {
            let directory_target = candidate.join("tsconfig.json");
            if directory_target.exists() {
                return Ok(Some(directory_target));
            }
            return Ok(None);
        }

        return Ok(Some(candidate.to_path_buf()));
    }

    if candidate.extension().is_none() {
        let file_target = candidate.with_extension("json");
        if file_target.exists() {
            return Ok(Some(file_target));
        }
    }

    let directory_target = candidate.join("tsconfig.json");
    if directory_target.exists() {
        return Ok(Some(directory_target));
    }

    Ok(None)
}

fn normalize_tsconfig_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_comparable_alias_target(base_dir: &Path, target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() || has_url_like_prefix(trimmed) {
        return None;
    }

    let comparable = if is_path_like_target(trimmed) {
        normalize_path_like_target(base_dir, trimmed)
    } else {
        trimmed.replace('\\', "/")
    };

    let comparable = strip_trailing_slashes(&comparable);
    if comparable.is_empty() {
        None
    } else {
        Some(comparable)
    }
}

fn normalize_path_like_target(base_dir: &Path, target: &str) -> String {
    let target = target.replace('\\', "/");
    let stripped = target.replace('*', "");
    let path = if Path::new(&stripped).is_absolute() {
        PathBuf::from(stripped)
    } else {
        base_dir.join(stripped)
    };

    normalize_path(&path)
}

fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();
    let mut prefix = None;

    for component in path.components() {
        match component {
            std::path::Component::Prefix(value) => {
                prefix = Some(value.as_os_str().to_string_lossy().replace('\\', "/"));
            }
            std::path::Component::RootDir => {
                components.clear();
                components.push(String::new());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if components.len() > 1 {
                    components.pop();
                }
            }
            std::path::Component::Normal(value) => {
                components.push(value.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    let mut output = String::new();
    if let Some(prefix) = prefix {
        output.push_str(&prefix);
    }
    if components.first().is_some_and(|segment| segment.is_empty()) {
        output.push('/');
        for segment in components.iter().skip(1) {
            if !output.ends_with('/') {
                output.push('/');
            }
            output.push_str(segment);
        }
    } else {
        for (index, segment) in components.iter().enumerate() {
            if index > 0 {
                output.push('/');
            }
            output.push_str(segment);
        }
    }

    strip_trailing_slashes(&output)
}

fn strip_trailing_slashes(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn normalize_alias_key(alias: &str) -> String {
    alias
        .trim()
        .trim_end_matches("/*")
        .trim_end_matches('*')
        .trim_end_matches('/')
        .to_string()
}

fn is_path_like_target(target: &str) -> bool {
    target.starts_with('.')
        || target.starts_with('/')
        || target.contains('/')
        || target.contains('\\')
        || target.contains('*')
}

fn has_url_like_prefix(target: &str) -> bool {
    let Some((prefix, _)) = target.split_once(':') else {
        return false;
    };

    !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn skip_template_literal(text: &str, start: usize) -> Option<usize> {
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '`' {
            return Some(cursor);
        }
    }

    None
}

fn parse_string_map_from_object(
    text: &str,
    object_start: usize,
    config_path: &Path,
) -> Option<BTreeMap<String, String>> {
    let object_end = find_matching_brace(text, object_start)?;
    let mut aliases = BTreeMap::new();
    let mut cursor = object_start + 1;

    while cursor < object_end {
        cursor = skip_ws_and_comments(text, cursor);
        if cursor >= object_end {
            break;
        }
        if text[cursor..].starts_with('}') {
            break;
        }

        let (key, next_cursor) = parse_string_or_identifier(text, cursor)?;
        let mut entry_cursor = skip_ws_and_comments(text, next_cursor);
        let Some(':') = text[entry_cursor..].chars().next() else {
            return None;
        };
        entry_cursor = skip_ws_and_comments(text, entry_cursor + 1);
        let Some((value, next_cursor)) = parse_alias_value(text, entry_cursor) else {
            cursor = skip_value(text, entry_cursor, object_end)?;
            continue;
        };

        let config_dir = config_path.parent().unwrap_or(config_path);
        if let Some(normalized_value) = normalize_comparable_alias_target(config_dir, &value) {
            aliases.insert(normalize_alias_key(&key), normalized_value);
        }

        cursor = skip_ws_and_comments(text, next_cursor);
        if cursor < object_end {
            match text[cursor..].chars().next() {
                Some(',') => {
                    cursor += 1;
                }
                Some('}') => {}
                _ => return None,
            }
        }
    }

    Some(aliases)
}

fn parse_alias_value(text: &str, cursor: usize) -> Option<(String, usize)> {
    if matches!(text[cursor..].chars().next(), Some('\'' | '"')) {
        return parse_string_literal(text, cursor);
    }

    parse_path_resolve_value(text, cursor)
}

fn parse_path_resolve_value(text: &str, cursor: usize) -> Option<(String, usize)> {
    let (callee, mut call_cursor) = parse_dotted_identifier(text, cursor)?;
    if callee != "path.resolve" && callee != "resolve" {
        return None;
    }

    call_cursor = skip_ws_and_comments(text, call_cursor);
    if !text[call_cursor..].starts_with('(') {
        return None;
    }
    let call_end = find_matching_delimiter(text, call_cursor, '(', ')')?;
    let mut args_cursor = call_cursor + 1;
    let mut segments = Vec::new();

    while args_cursor < call_end {
        args_cursor = skip_ws_and_comments(text, args_cursor);
        if args_cursor >= call_end {
            break;
        }

        if matches!(text[args_cursor..].chars().next(), Some('\'' | '"')) {
            let (segment, next_cursor) = parse_string_literal(text, args_cursor)?;
            if Path::new(&segment).is_absolute() {
                segments.clear();
            }
            segments.push(segment);
            args_cursor = next_cursor;
        } else {
            args_cursor = skip_value(text, args_cursor, call_end)?;
        }

        args_cursor = skip_ws_and_comments(text, args_cursor);
        if args_cursor < call_end && text[args_cursor..].starts_with(',') {
            args_cursor += 1;
        }
    }

    if segments.is_empty() {
        return None;
    }

    Some((segments.join("/"), call_end + 1))
}

fn parse_dotted_identifier(text: &str, start: usize) -> Option<(String, usize)> {
    let (mut value, mut cursor) = parse_identifier(text, start)?;

    loop {
        let dot_cursor = skip_ws_and_comments(text, cursor);
        if !text[dot_cursor..].starts_with('.') {
            break;
        }
        let (part, next_cursor) = parse_identifier(text, dot_cursor + 1)?;
        value.push('.');
        value.push_str(&part);
        cursor = next_cursor;
    }

    Some((value, cursor))
}

fn parse_string_or_identifier(text: &str, start: usize) -> Option<(String, usize)> {
    let Some(ch) = text[start..].chars().next() else {
        return None;
    };
    if ch == '\'' || ch == '"' {
        parse_string_literal(text, start)
    } else {
        parse_identifier(text, start)
    }
}

fn parse_identifier(text: &str, start: usize) -> Option<(String, usize)> {
    let mut cursor = start;
    let mut identifier = String::new();
    for ch in text[start..].chars() {
        if identifier.is_empty() {
            if !is_identifier_start(ch) {
                return None;
            }
        } else if !is_identifier_char(ch) {
            break;
        }

        identifier.push(ch);
        cursor += ch.len_utf8();
    }

    if identifier.is_empty() {
        None
    } else {
        Some((identifier, cursor))
    }
}

fn parse_string_literal(text: &str, start: usize) -> Option<(String, usize)> {
    let Some(quote) = text[start..].chars().next() else {
        return None;
    };
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut cursor = start + quote.len_utf8();
    let mut value = String::new();
    let mut escaped = false;
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some((value, cursor));
        }
        value.push(ch);
    }

    None
}

fn find_matching_brace(text: &str, start: usize) -> Option<usize> {
    find_matching_delimiter(text, start, '{', '}')
}

fn find_matching_delimiter(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut cursor = start;
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => in_string = Some(ch),
            candidate if candidate == open => {
                depth += 1;
            }
            candidate if candidate == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(cursor - ch.len_utf8());
                }
            }
            '/' if text[cursor..].starts_with('/') => {
                while cursor < text.len() {
                    let next = text[cursor..].chars().next()?;
                    cursor += next.len_utf8();
                    if next == '\n' {
                        break;
                    }
                }
            }
            '/' if text[cursor..].starts_with('*') => {
                cursor += 1;
                while cursor < text.len() {
                    let next = text[cursor..].chars().next()?;
                    cursor += next.len_utf8();
                    if next == '*' && text[cursor..].starts_with('/') {
                        cursor += 1;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn skip_ws_and_comments(text: &str, mut cursor: usize) -> usize {
    while cursor < text.len() {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }
        if ch == '/' {
            let next = text[cursor + ch.len_utf8()..].chars().next();
            match next {
                Some('/') => {
                    cursor += 2;
                    while cursor < text.len() {
                        let Some(next_ch) = text[cursor..].chars().next() else {
                            break;
                        };
                        cursor += next_ch.len_utf8();
                        if next_ch == '\n' {
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    cursor += 2;
                    while cursor < text.len() {
                        let Some(next_ch) = text[cursor..].chars().next() else {
                            break;
                        };
                        cursor += next_ch.len_utf8();
                        if next_ch == '*' && text[cursor..].starts_with('/') {
                            cursor += 1;
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        break;
    }

    cursor
}

fn is_identifier_start(ch: char) -> bool {
    ch == '$' || ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_char(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}
