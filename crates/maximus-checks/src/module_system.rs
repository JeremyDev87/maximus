use std::io;

use maximus_core::{make_finding, FindingInput, ProjectSnapshot, Severity};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;
use crate::registry::{
    package_file_for_directory, read_effective_compiler_options, read_package_json,
    tsconfig_entry_file_for_directory,
};

pub fn run_module_system_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let Some(package_file) = package_file_for_directory(directory) else {
            continue;
        };
        let Some(package_json) = read_package_json(&package_file.path) else {
            continue;
        };
        let Some(package_type) = package_json.get("type").and_then(Value::as_str) else {
            continue;
        };
        let Some(tsconfig_file) = tsconfig_entry_file_for_directory(directory) else {
            continue;
        };
        let Some(compiler_options) = read_effective_compiler_options(&tsconfig_file.path)? else {
            continue;
        };

        let module_setting = compiler_options.get("module").and_then(Value::as_str);

        if let Some((severity, title, detail, hint)) =
            module_system_finding(package_type, module_setting)
        {
            findings.push(make_finding(FindingInput {
                id: format!("module-system:{}", tsconfig_file.path.to_string_lossy()),
                title: title.to_string(),
                category: Some("module-system".to_string()),
                detail: Some(detail),
                file: Some(tsconfig_file.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(hint.to_string()),
                severity: Some(severity),
            }));
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn module_system_finding(
    package_type: &str,
    module_setting: Option<&str>,
) -> Option<(Severity, &'static str, String, &'static str)> {
    match (package_type, module_setting) {
        ("module", Some("commonjs")) | ("module", Some("amd")) | ("module", Some("umd")) => Some((
            Severity::Error,
            "Package ESM type conflicts with tsconfig module output",
            format!(
                "package.json type is \"module\", but compilerOptions.module is {:?}.",
                module_setting.unwrap()
            ),
            "Use an ESM-aware module target or switch package.json type back to commonjs so the runtime and compiler agree.",
        )),
        ("commonjs", Some("esnext")) | ("commonjs", Some("preserve")) => Some((
            Severity::Warn,
            "Package CommonJS type conflicts with tsconfig module output",
            format!(
                "package.json type is \"commonjs\", but compilerOptions.module is {:?}.",
                module_setting.unwrap()
            ),
            "Use a CommonJS module target or update package.json type to match the emitted module system.",
        )),
        _ => None,
    }
}
