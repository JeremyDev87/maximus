#![cfg_attr(not(test), allow(dead_code))]

use std::path::Path;

use maximus_core::{AppliedFix, AuditResult, PreviewedFix, Severity};
use serde_json::{json, Value};

pub fn render_audit_result(result: &AuditResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_log("audit", result, None, None, None, None))
}

pub fn render_doctor_result(result: &AuditResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_log("doctor", result, None, None, None, None))
}

pub fn render_fix_result(
    dry_run: bool,
    target_dir: &Path,
    initial: &AuditResult,
    applied: &[AppliedFix],
    final_result: &AuditResult,
    preview: Option<&[PreviewedFix]>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&build_fix_log(
        dry_run,
        target_dir,
        initial,
        applied,
        final_result,
        preview,
    ))
}

fn build_log(
    kind: &str,
    result: &AuditResult,
    dry_run: Option<bool>,
    target_dir: Option<&Path>,
    applied: Option<&[AppliedFix]>,
    preview: Option<&[PreviewedFix]>,
) -> Value {
    json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "maximus",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            },
            "results": result.findings.iter().map(|finding| finding_to_result(&result.root_dir, finding)).collect::<Vec<_>>(),
            "properties": {
                "reportKind": kind,
                "rootDir": result.root_dir,
                "summary": result.summary,
                "structure": result.structure,
                "dryRun": dry_run,
                "targetDir": target_dir.map(display_path),
                "applied": applied.map(serializable_applied_fixes),
                "preview": preview.map(serializable_previewed_fixes),
            }
        }]
    })
}

fn build_fix_log(
    dry_run: bool,
    target_dir: &Path,
    initial: &AuditResult,
    applied: &[AppliedFix],
    final_result: &AuditResult,
    preview: Option<&[PreviewedFix]>,
) -> Value {
    json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "maximus",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            },
            "results": final_result.findings.iter().map(|finding| finding_to_result(&final_result.root_dir, finding)).collect::<Vec<_>>(),
            "properties": {
                "reportKind": "fix",
                "dryRun": dry_run,
                "targetDir": display_path(target_dir),
                "initial": {
                    "rootDir": initial.root_dir,
                    "summary": initial.summary,
                    "structure": initial.structure,
                },
                "final": {
                    "rootDir": final_result.root_dir,
                    "summary": final_result.summary,
                    "structure": final_result.structure,
                },
                "applied": serializable_applied_fixes(applied),
                "preview": preview.map(serializable_previewed_fixes),
            }
        }]
    })
}

fn finding_to_result(root_dir: &Path, finding: &maximus_core::Finding) -> Value {
    let level = match finding.severity {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
    };

    json!({
        "ruleId": finding.category,
        "level": level,
        "message": {
            "text": if finding.detail.is_empty() {
                finding.title.clone()
            } else {
                format!("{}: {}", finding.title, finding.detail)
            }
        },
        "locations": finding.file.as_ref().map(|file| vec![json!({
            "physicalLocation": {
                "artifactLocation": {
                    "uri": format_relative_file(root_dir, file),
                }
            }
        })]).unwrap_or_default(),
        "properties": {
            "findingId": finding.id,
            "category": finding.category,
            "title": finding.title,
            "detail": finding.detail,
            "hint": finding.hint,
            "fixable": finding.fixable,
            "fixIds": finding.fix_ids,
        }
    })
}

fn serializable_applied_fixes(applied: &[AppliedFix]) -> Vec<Value> {
    applied
        .iter()
        .map(|fix| {
            json!({
                "id": fix.id,
                "title": fix.title,
                "files": fix.files,
                "outcome": fix.outcome,
            })
        })
        .collect()
}

fn serializable_previewed_fixes(previews: &[PreviewedFix]) -> Vec<Value> {
    previews
        .iter()
        .map(|preview| {
            json!({
                "id": preview.id,
                "title": preview.title,
                "files": preview.files,
                "diffs": preview.previews.iter().map(|file| json!({
                    "path": file.path,
                    "existedBefore": file.existed_before,
                    "before": file.before,
                    "after": file.after,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn format_relative_file(root_dir: &Path, file_path: &Path) -> String {
    relative_path_like_js(root_dir, file_path)
        .map(|value| {
            if value.is_empty() {
                ".".to_string()
            } else {
                value
            }
        })
        .unwrap_or_else(|| display_path(file_path))
}

fn relative_path_like_js(root_dir: &Path, file_path: &Path) -> Option<String> {
    if root_dir == file_path {
        return Some(String::new());
    }

    if let Ok(relative) = file_path.strip_prefix(root_dir) {
        return Some(path_string(relative));
    }

    let root_components = root_dir.components().collect::<Vec<_>>();
    let file_components = file_path.components().collect::<Vec<_>>();

    let mut shared_len = 0usize;
    while shared_len < root_components.len()
        && shared_len < file_components.len()
        && root_components[shared_len] == file_components[shared_len]
    {
        shared_len += 1;
    }

    if shared_len == 0 {
        return None;
    }

    let mut relative = std::path::PathBuf::new();

    for component in &root_components[shared_len..] {
        if matches!(component, std::path::Component::Normal(_)) {
            relative.push("..");
        }
    }

    for component in &file_components[shared_len..] {
        relative.push(component.as_os_str());
    }

    Some(path_string(&relative))
}

fn display_path(path: &Path) -> String {
    path_string(path)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use maximus_core::{
        AppliedFix, AuditResult, AuditSummary, Finding, PreviewedFix, Severity, StructureReport,
    };
    use serde_json::Value;

    use super::{render_audit_result, render_doctor_result, render_fix_result};

    #[test]
    fn audit_sarif_contains_expected_top_level_shape() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_audit_result(&result).expect("sarif should render");
        let value: Value = serde_json::from_str(&json).expect("sarif should parse");

        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "maximus");
        assert_eq!(value["runs"][0]["properties"]["reportKind"], "audit");
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn doctor_sarif_labels_report_kind_as_doctor() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_doctor_result(&result).expect("sarif should render");
        let value: Value = serde_json::from_str(&json).expect("sarif should parse");

        assert_eq!(value["runs"][0]["properties"]["reportKind"], "doctor");
    }

    #[test]
    fn fix_sarif_includes_fix_properties() {
        let root_dir = PathBuf::from("/tmp/project");
        let result = AuditResult {
            root_dir: root_dir.clone(),
            summary: AuditSummary {
                status: "clean".to_string(),
                total_findings: 0,
                blocking_findings: 0,
                warning_findings: 0,
                info_findings: 0,
                fixable_findings: 0,
                fixes_available: 0,
                config_files: 1,
                package_count: 1,
                env_directories: 0,
            },
            structure: StructureReport {
                is_monorepo: false,
                package_count: 1,
                env_directories: 0,
                config_files: 1,
                recommendations: Vec::new(),
            },
            findings: vec![Finding {
                id: "env-1".to_string(),
                severity: Severity::Warn,
                category: "env".to_string(),
                title: "Missing example".to_string(),
                detail: "create .env.example".to_string(),
                file: Some(root_dir.join(".env.example")),
                hint: "run maximus fix".to_string(),
                fixable: true,
                fix_ids: vec!["env-create-example".to_string()],
            }],
            fixes: Vec::new(),
        };

        let json = render_fix_result(
            true,
            &root_dir,
            &result,
            &[AppliedFix {
                id: "env-1".to_string(),
                title: "Create .env.example".to_string(),
                files: vec![root_dir.join(".env.example")],
                outcome: "created".to_string(),
            }],
            &result,
            Some(&[PreviewedFix {
                id: "env-1".to_string(),
                title: "Create .env.example".to_string(),
                files: vec![root_dir.join(".env.example")],
                previews: vec![maximus_core::FixFilePreview {
                    path: root_dir.join(".env.example"),
                    existed_before: false,
                    before: String::new(),
                    after: "API_URL=\n".to_string(),
                }],
            }]),
        )
        .expect("sarif should render");
        let value: Value = serde_json::from_str(&json).expect("sarif should parse");

        assert_eq!(value["runs"][0]["properties"]["reportKind"], "fix");
        assert_eq!(value["runs"][0]["properties"]["dryRun"], true);
        assert_eq!(
            value["runs"][0]["properties"]["applied"][0]["outcome"],
            "created"
        );
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "env");
    }

    fn sample_result(root_dir: PathBuf) -> AuditResult {
        AuditResult {
            root_dir,
            summary: AuditSummary {
                status: "clean".to_string(),
                total_findings: 0,
                blocking_findings: 0,
                warning_findings: 0,
                info_findings: 0,
                fixable_findings: 0,
                fixes_available: 0,
                config_files: 1,
                package_count: 1,
                env_directories: 0,
            },
            structure: StructureReport {
                is_monorepo: false,
                package_count: 1,
                env_directories: 0,
                config_files: 1,
                recommendations: vec![
                    "Current config surface looks healthy. Keep shared rules centralized as the repo grows."
                        .to_string(),
                ],
            },
            findings: Vec::new(),
            fixes: Vec::new(),
        }
    }
}
