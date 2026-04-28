use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use maximus_core::{
    make_finding, parse_jsonc, read_text_if_exists, FileKind, FindingInput, ProjectFile,
    ProjectSnapshot, Severity,
};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;
use crate::registry::{package_file_for_directory, read_package_json};

#[derive(Debug)]
struct EditorConfigDocument {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

#[derive(Debug)]
struct PrettierDocument {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

pub fn run_editorconfig_prettier_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let editorconfigs = find_editorconfig_documents(&project.root_dir)?;
    if editorconfigs.is_empty() {
        return Ok(CheckOutcome {
            findings: Vec::new(),
            fixes: Vec::new(),
            planned_fixes: Vec::new(),
        });
    }

    let prettier_documents = find_prettier_documents(project)?;
    let editorconfig = &editorconfigs[0];
    let mut findings = Vec::new();
    let mut reported_directories = BTreeSet::new();

    for prettier in prettier_documents {
        let conflicts = find_conflicts(&editorconfig.values, &prettier.values);
        if conflicts.is_empty() {
            continue;
        }

        let directory = prettier
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project.root_dir.clone());
        if !reported_directories.insert(directory.clone()) {
            continue;
        }

        let editor_detail = conflicts
            .iter()
            .map(|conflict| {
                format!(
                    "{}={}",
                    conflict.editorconfig_key, conflict.editorconfig_value
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let prettier_detail = conflicts
            .iter()
            .map(|conflict| format!("{}={}", conflict.prettier_key, conflict.prettier_value))
            .collect::<Vec<_>>()
            .join(", ");

        findings.push(make_finding(FindingInput {
            id: format!(
                "editorconfig-prettier-conflict:{}",
                directory.to_string_lossy()
            ),
            title: "EditorConfig and Prettier disagree".to_string(),
            category: Some("editorconfig-prettier".to_string()),
            detail: Some(format!(
                "EditorConfig sets {editor_detail}, but Prettier sets {prettier_detail}."
            )),
            file: Some(editorconfig.path.clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Align EditorConfig and Prettier so editor saves do not fight formatter output."
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

#[derive(Debug)]
struct Conflict {
    editorconfig_key: &'static str,
    editorconfig_value: String,
    prettier_key: &'static str,
    prettier_value: String,
}

fn find_conflicts(
    editorconfig: &BTreeMap<String, String>,
    prettier: &BTreeMap<String, String>,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    if let (Some(indent_style), Some(use_tabs)) =
        (editorconfig.get("indent_style"), prettier.get("useTabs"))
    {
        let expected_use_tabs = match indent_style.as_str() {
            "tab" => Some("true"),
            "space" => Some("false"),
            _ => None,
        };
        if expected_use_tabs.is_some_and(|expected| expected != use_tabs) {
            conflicts.push(Conflict {
                editorconfig_key: "indent_style",
                editorconfig_value: indent_style.clone(),
                prettier_key: "useTabs",
                prettier_value: use_tabs.clone(),
            });
        }
    }

    if let (Some(indent_size), Some(tab_width)) =
        (editorconfig.get("indent_size"), prettier.get("tabWidth"))
    {
        if indent_size != "tab"
            && parse_positive_number(indent_size) != parse_positive_number(tab_width)
        {
            conflicts.push(Conflict {
                editorconfig_key: "indent_size",
                editorconfig_value: indent_size.clone(),
                prettier_key: "tabWidth",
                prettier_value: tab_width.clone(),
            });
        }
    }

    if let (Some(end_of_line), Some(prettier_end_of_line)) =
        (editorconfig.get("end_of_line"), prettier.get("endOfLine"))
    {
        if prettier_end_of_line != "auto" && end_of_line != prettier_end_of_line {
            conflicts.push(Conflict {
                editorconfig_key: "end_of_line",
                editorconfig_value: end_of_line.clone(),
                prettier_key: "endOfLine",
                prettier_value: prettier_end_of_line.clone(),
            });
        }
    }

    if let (Some(max_line_length), Some(print_width)) = (
        editorconfig.get("max_line_length"),
        prettier.get("printWidth"),
    ) {
        if max_line_length != "off"
            && parse_positive_number(max_line_length) != parse_positive_number(print_width)
        {
            conflicts.push(Conflict {
                editorconfig_key: "max_line_length",
                editorconfig_value: max_line_length.clone(),
                prettier_key: "printWidth",
                prettier_value: print_width.clone(),
            });
        }
    }

    conflicts
}

fn find_editorconfig_documents(root_dir: &Path) -> io::Result<Vec<EditorConfigDocument>> {
    let path = root_dir.join(".editorconfig");
    let Some(text) = read_text_if_exists(&path)? else {
        return Ok(Vec::new());
    };

    let values = parse_editorconfig_values(&text);
    if values.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![EditorConfigDocument { path, values }])
}

fn find_prettier_documents(project: &ProjectSnapshot) -> io::Result<Vec<PrettierDocument>> {
    let mut documents = Vec::new();

    for directory in &project.directories {
        if let Some(package_file) = package_file_for_directory(directory) {
            if let Some(prettier) = read_package_prettier(package_file) {
                documents.push(prettier);
            }
        }

        if let Some(prettier_files) = directory.files_by_kind.get(&FileKind::Prettier) {
            for file in prettier_files {
                if let Some(prettier) = read_prettier_file(file)? {
                    documents.push(prettier);
                }
            }
        }
    }

    Ok(documents)
}

fn read_package_prettier(package_file: &ProjectFile) -> Option<PrettierDocument> {
    let package_json = read_package_json(&package_file.path)?;
    let prettier = package_json.get("prettier")?;
    let values = prettier_values_from_json(prettier);
    if values.is_empty() {
        return None;
    }

    Some(PrettierDocument {
        path: package_file.path.clone(),
        values,
    })
}

fn read_prettier_file(file: &ProjectFile) -> io::Result<Option<PrettierDocument>> {
    let Some(text) = read_text_if_exists(&file.path)? else {
        return Ok(None);
    };

    let values = parse_jsonc::<Value>(&text, &file.path.to_string_lossy())
        .ok()
        .map(|value| prettier_values_from_json(&value))
        .unwrap_or_else(|| prettier_values_from_text(&text));

    if values.is_empty() {
        return Ok(None);
    }

    Ok(Some(PrettierDocument {
        path: file.path.clone(),
        values,
    }))
}

fn prettier_values_from_json(value: &Value) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let Some(object) = value.as_object() else {
        return values;
    };

    for key in ["useTabs", "tabWidth", "endOfLine", "printWidth"] {
        if let Some(value) = object.get(key).and_then(normalize_prettier_value) {
            values.insert(key.to_string(), value);
        }
    }

    values
}

fn prettier_values_from_text(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for key in ["useTabs", "tabWidth", "endOfLine", "printWidth"] {
        if let Some(value) = find_js_property_value(text, key) {
            values.insert(key.to_string(), value);
        }
    }
    values
}

fn normalize_prettier_value(value: &Value) -> Option<String> {
    match value {
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.to_ascii_lowercase()),
        _ => None,
    }
}

fn parse_editorconfig_values(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();

    for line in text.lines() {
        let line = strip_editorconfig_comment(line).trim();
        if line.is_empty() || line.starts_with('[') {
            if line.starts_with('[') {
                break;
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(
            key.trim().to_ascii_lowercase(),
            value.trim().to_ascii_lowercase(),
        );
    }

    values
}

fn strip_editorconfig_comment(line: &str) -> &str {
    line.split(['#', ';']).next().unwrap_or(line)
}

fn parse_positive_number(value: &str) -> Option<u64> {
    value.parse::<u64>().ok()
}

fn find_js_property_value(text: &str, property: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{quote}{property}{quote}");
        if let Some(value) = find_js_property_value_after(text, &needle) {
            return Some(value);
        }
    }

    find_js_property_value_after(text, property)
}

fn find_js_property_value_after(text: &str, needle: &str) -> Option<String> {
    let index = text.find(needle)?;
    let after_key = &text[index + needle.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    parse_simple_js_value(after_colon)
}

fn parse_simple_js_value(text: &str) -> Option<String> {
    let first = text.chars().next()?;
    if first == '"' || first == '\'' {
        let rest = &text[first.len_utf8()..];
        let end = rest.find(first)?;
        return Some(rest[..end].to_ascii_lowercase());
    }

    let value = text
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
