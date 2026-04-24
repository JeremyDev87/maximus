use std::io;

use maximus_core::{
    make_finding, parse_jsonc, read_text_if_exists, FileKind, FindingInput, ProjectSnapshot,
    Severity,
};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;

pub fn run_workspace_config_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let Some(workspace_files) = directory.files_by_kind.get(&FileKind::Workspace) else {
            continue;
        };

        for workspace_file in workspace_files {
            match workspace_file.name.as_str() {
                "pnpm-workspace.yaml" => {
                    if let Some(finding) = inspect_pnpm_workspace_file(project, workspace_file)? {
                        findings.push(finding);
                    }
                }
                "turbo.json" => {
                    if let Some(finding) = inspect_turbo_workspace_file(workspace_file)? {
                        findings.push(finding);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn inspect_pnpm_workspace_file(
    project: &ProjectSnapshot,
    workspace_file: &maximus_core::ProjectFile,
) -> io::Result<Option<maximus_core::Finding>> {
    let Some(text) = read_text_if_exists(&workspace_file.path)? else {
        return Ok(None);
    };

    let parse_result = parse_pnpm_workspace_packages(&text);
    let Ok(package_patterns) = parse_result else {
        return Ok(Some(make_workspace_finding(
            workspace_file,
            "pnpm-workspace.yaml could not be parsed",
            "The file uses a pnpm-workspace.yaml shape that this check does not understand.",
            "Use a simple packages: block list such as packages: [\"apps/*\", \"packages/*\"].",
        )));
    };

    if !package_patterns.is_empty() {
        return Ok(None);
    }

    let detail = if project.package_files.len() <= 1 {
        "No package globs were found under packages:, and this repo has at most one package file, so the workspace file looks like a placeholder."
            .to_string()
    } else {
        "No package globs were found under packages:, so workspace packages are not declared yet."
            .to_string()
    };

    Ok(Some(make_workspace_finding(
        workspace_file,
        "pnpm-workspace.yaml does not declare any package patterns",
        &detail,
        "Add a packages: block with one or more workspace globs, or remove the file until the repo actually needs a workspace definition.",
    )))
}

fn inspect_turbo_workspace_file(
    workspace_file: &maximus_core::ProjectFile,
) -> io::Result<Option<maximus_core::Finding>> {
    let Some(text) = read_text_if_exists(&workspace_file.path)? else {
        return Ok(None);
    };
    let Ok(config) = parse_jsonc::<Value>(&text, &workspace_file.path.to_string_lossy()) else {
        return Ok(Some(make_workspace_finding(
            workspace_file,
            "turbo.json could not be parsed",
            "The turbo.json file is not valid JSONC.",
            "Fix the syntax error or replace the placeholder file with a real Turbo config.",
        )));
    };

    if turbo_workspace_is_substantial(&config) {
        return Ok(None);
    }

    Ok(Some(make_workspace_finding(
        workspace_file,
        "turbo.json does not declare any workspace tasks",
        "The file does not contain any task definitions that would make the workspace config meaningful.",
        "Add a non-empty tasks or pipeline map, or remove the placeholder turbo.json until the workspace needs it.",
    )))
}

fn parse_pnpm_workspace_packages(text: &str) -> Result<Vec<String>, ()> {
    let mut packages = Vec::new();
    let mut lines = text.lines().enumerate().peekable();
    let mut saw_packages_key = false;

    while let Some((_, line)) = lines.next() {
        let trimmed = strip_inline_comment(line).trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        if !saw_packages_key {
            if let Some(rest) = trimmed.trim_start().strip_prefix("packages:") {
                saw_packages_key = true;
                let rest = rest.trim();
                if rest == "[]" {
                    return Ok(packages);
                }
                if rest.starts_with('[') {
                    return parse_inline_package_array(rest);
                }
                if !rest.is_empty() {
                    return Err(());
                }

                let base_indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
                while let Some((_, next_line)) = lines.peek() {
                    let next_trimmed = strip_inline_comment(next_line).trim_end();
                    if next_trimmed.trim().is_empty() {
                        lines.next();
                        continue;
                    }

                    let next_indent = next_line
                        .chars()
                        .take_while(|ch| ch.is_whitespace())
                        .count();
                    if next_indent <= base_indent {
                        break;
                    }

                    let content = next_trimmed.trim_start();
                    if let Some(item) = content.strip_prefix("- ") {
                        let item = item.trim();
                        if item.starts_with('[') || item.starts_with('{') {
                            return Err(());
                        }
                        if !item.is_empty() {
                            packages.push(item.to_string());
                        }
                        lines.next();
                        continue;
                    }

                    if content.starts_with('-') {
                        return Err(());
                    }

                    return Err(());
                }
            }
        }
    }

    if !saw_packages_key {
        return Ok(packages);
    }

    Ok(packages)
}

fn parse_inline_package_array(value: &str) -> Result<Vec<String>, ()> {
    let mut cursor = skip_inline_ws(value, 0);
    if value[cursor..].chars().next() != Some('[') {
        return Err(());
    }
    cursor += 1;

    let mut packages = Vec::new();
    loop {
        cursor = skip_inline_ws(value, cursor);
        let Some(ch) = value[cursor..].chars().next() else {
            return Err(());
        };
        if ch == ']' {
            return Ok(packages);
        }

        let (package, next_cursor) = parse_inline_array_item(value, cursor)?;
        packages.push(package);
        cursor = skip_inline_ws(value, next_cursor);

        match value[cursor..].chars().next() {
            Some(',') => {
                cursor += 1;
            }
            Some(']') => return Ok(packages),
            _ => return Err(()),
        }
    }
}

fn parse_inline_array_item(value: &str, start: usize) -> Result<(String, usize), ()> {
    match value[start..].chars().next() {
        Some('\'') | Some('"') => parse_inline_string_literal(value, start),
        Some('[') | Some('{') | None => Err(()),
        Some(_) => parse_inline_unquoted_scalar(value, start),
    }
}

fn parse_inline_unquoted_scalar(value: &str, start: usize) -> Result<(String, usize), ()> {
    let mut cursor = start;
    while cursor < value.len() {
        let Some(ch) = value[cursor..].chars().next() else {
            break;
        };
        if ch == ',' || ch == ']' {
            break;
        }
        cursor += ch.len_utf8();
    }

    let item = value[start..cursor].trim();
    if item.is_empty() {
        return Err(());
    }

    Ok((item.to_string(), cursor))
}

fn parse_inline_string_literal(value: &str, start: usize) -> Result<(String, usize), ()> {
    let Some(quote) = value[start..].chars().next() else {
        return Err(());
    };
    if quote != '\'' && quote != '"' {
        return Err(());
    }

    let mut cursor = start + quote.len_utf8();
    let mut output = String::new();
    let mut escaped = false;

    while cursor < value.len() {
        let Some(ch) = value[cursor..].chars().next() else {
            break;
        };
        cursor += ch.len_utf8();

        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Ok((output, cursor));
        }
        output.push(ch);
    }

    Err(())
}

fn skip_inline_ws(value: &str, mut cursor: usize) -> usize {
    while cursor < value.len() {
        let Some(ch) = value[cursor..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }
        break;
    }

    cursor
}

fn turbo_workspace_is_substantial(config: &Value) -> bool {
    let Some(object) = config.as_object() else {
        return false;
    };

    for key in ["tasks", "pipeline"] {
        if object
            .get(key)
            .and_then(Value::as_object)
            .map(|entries| !entries.is_empty())
            .unwrap_or(false)
        {
            return true;
        }
    }

    false
}

fn make_workspace_finding(
    workspace_file: &maximus_core::ProjectFile,
    title: &str,
    detail: &str,
    hint: &str,
) -> maximus_core::Finding {
    make_finding(FindingInput {
        id: format!("workspace-config:{}", workspace_file.path.to_string_lossy()),
        title: title.to_string(),
        category: Some("workspace-config".to_string()),
        detail: Some(detail.to_string()),
        file: Some(workspace_file.path.clone()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some(hint.to_string()),
        severity: Some(Severity::Warn),
    })
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return &line[..index],
            _ => {}
        }
    }

    line
}
