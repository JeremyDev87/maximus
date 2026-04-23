use std::io;

use maximus_core::{make_finding, FindingInput, ProjectSnapshot, Severity};
use serde_json::Value;

use crate::check_outcome::CheckOutcome;
use crate::registry::{
    package_file_for_directory, read_effective_compiler_options, read_package_json,
    tsconfig_entry_file_for_directory,
};

#[derive(Clone, Copy)]
enum FrameworkRuntime {
    Preact,
    Solid,
}

impl FrameworkRuntime {
    fn dependency_name(self) -> &'static str {
        match self {
            FrameworkRuntime::Preact => "preact",
            FrameworkRuntime::Solid => "solid-js",
        }
    }

    fn jsx_import_source(self) -> &'static str {
        self.dependency_name()
    }
}

pub fn run_jsx_config_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for directory in &project.directories {
        let Some(package_file) = package_file_for_directory(directory) else {
            continue;
        };
        let Some(package_json) = read_package_json(&package_file.path) else {
            continue;
        };
        let Some(framework_runtime) = detect_framework_runtime(&package_json) else {
            continue;
        };
        let Some(tsconfig_file) = tsconfig_entry_file_for_directory(directory) else {
            continue;
        };
        let Some(compiler_options) = read_effective_compiler_options(&tsconfig_file.path)? else {
            continue;
        };

        let jsx_mode = compiler_options.get("jsx").and_then(Value::as_str);
        if !uses_automatic_jsx_runtime(jsx_mode) {
            continue;
        }

        let jsx_import_source = compiler_options
            .get("jsxImportSource")
            .and_then(Value::as_str);

        if jsx_import_source == Some(framework_runtime.jsx_import_source()) {
            continue;
        }

        let existing_import_source = jsx_import_source
            .map(|source| format!("It currently points at {source}."))
            .unwrap_or_else(|| "No jsxImportSource is configured yet.".to_string());

        findings.push(make_finding(FindingInput {
            id: format!("jsx-config:{}", tsconfig_file.path.to_string_lossy()),
            title: format!(
                "{} JSX runtime should declare jsxImportSource",
                framework_runtime.dependency_name()
            ),
            category: Some("jsx-config".to_string()),
            detail: Some(format!(
                "package.json depends on {}, but compilerOptions.jsxImportSource is missing or different. {}",
                framework_runtime.dependency_name(),
                existing_import_source
            )),
            file: Some(tsconfig_file.path.clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(format!(
                "Set compilerOptions.jsxImportSource to \"{}\" so the JSX transform matches the framework runtime.",
                framework_runtime.jsx_import_source()
            )),
            severity: Some(Severity::Info),
        }));
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn detect_framework_runtime(package_json: &Value) -> Option<FrameworkRuntime> {
    for runtime in [FrameworkRuntime::Preact, FrameworkRuntime::Solid] {
        if package_depends_on(package_json, runtime.dependency_name()) {
            return Some(runtime);
        }
    }

    None
}

fn package_depends_on(package_json: &Value, dependency_name: &str) -> bool {
    [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ]
    .into_iter()
    .any(|section| {
        package_json
            .get(section)
            .and_then(Value::as_object)
            .map(|dependencies| dependencies.contains_key(dependency_name))
            .unwrap_or(false)
    })
}

fn uses_automatic_jsx_runtime(jsx_mode: Option<&str>) -> bool {
    matches!(jsx_mode, Some("react-jsx") | Some("react-jsxdev"))
}
