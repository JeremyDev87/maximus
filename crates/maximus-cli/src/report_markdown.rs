#![cfg_attr(not(test), allow(dead_code))]

use std::path::Path;

use maximus_core::{AppliedFix, AuditResult, FixPlan, Severity, StructureReport};

pub fn format_audit_report(result: &AuditResult) -> String {
    let mut lines = vec![
        "# Maximus audit".to_string(),
        String::new(),
        format!("- Target: `{}`", display_path(&result.root_dir)),
        format!("- Status: `{}`", result.summary.status),
        format!(
            "- Findings: `{}` error, `{}` warnings, `{}` info",
            result.summary.blocking_findings,
            result.summary.warning_findings,
            result.summary.info_findings
        ),
        format!("- Fixes available: `{}`", result.summary.fixes_available),
        String::new(),
        "## Structure".to_string(),
        format!("{}.", describe_structure(&result.structure)),
    ];

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("## Findings".to_string());
        lines.push("- No config drift detected.".to_string());
    } else {
        lines.push(String::new());
        lines.push("## Findings".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("## Recommendations".to_string());
        for recommendation in &result.structure.recommendations {
            lines.push(format!("- {recommendation}"));
        }
    }

    lines.join("\n")
}

pub fn format_doctor_report(result: &AuditResult) -> String {
    let mut lines = vec![
        "# Maximus doctor".to_string(),
        String::new(),
        format!("- Target: `{}`", display_path(&result.root_dir)),
        format!("- Diagnosis: `{}`", result.summary.status),
        format!("- Project shape: {}", describe_structure(&result.structure)),
        String::new(),
        "## Prescription".to_string(),
    ];

    let manual_findings = result
        .findings
        .iter()
        .filter(|finding| !finding.fixable)
        .count();
    let fixable_findings = result
        .findings
        .iter()
        .filter(|finding| finding.fixable)
        .count();

    if fixable_findings > 0 {
        lines.push(format!(
            "- Run `maximus fix` to apply {} safe fix(es).",
            result.summary.fixes_available
        ));
    } else {
        lines.push("- No automatic fixes are currently available.".to_string());
    }

    if manual_findings > 0 {
        lines.push(format!(
            "- Review {manual_findings} manual issue(s) in priority order below."
        ));
    } else {
        lines.push("- No manual follow-up is required right now.".to_string());
    }

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("## Findings".to_string());
        lines.push("- No config drift detected.".to_string());
    } else {
        lines.push(String::new());
        lines.push("## Top Priorities".to_string());
        lines.extend(format_top_priorities(result));
        lines.push(String::new());
        lines.push("## Findings".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("## Recommended Structure".to_string());
        for recommendation in &result.structure.recommendations {
            lines.push(format!("- {recommendation}"));
        }
    }

    lines.join("\n")
}

pub fn format_fix_result(
    dry_run: bool,
    target_dir: &Path,
    initial: &AuditResult,
    applied: &[AppliedFix],
    final_result: &AuditResult,
    selected_fixes: Option<&[FixPlan]>,
    preview_report: Option<&str>,
) -> String {
    let mut lines = vec![
        "# Maximus fix".to_string(),
        String::new(),
        format!("- Target: `{}`", display_path(target_dir)),
    ];

    if dry_run {
        if let Some(selected_fixes) = selected_fixes.filter(|fixes| !fixes.is_empty()) {
            lines.push(format!(
                "- Dry run: `{}` safe fix(es) selected.",
                selected_fixes.len()
            ));
        } else {
            lines.push(format!(
                "- Dry run: `{}` safe fix(es) available.",
                initial.summary.fixes_available
            ));
        }
    } else {
        lines.push(format!("- Applied: `{}` fix(es).", applied.len()));
    }

    if !applied.is_empty() {
        lines.push(String::new());
        lines.push("## Changes".to_string());
        for fix in applied {
            lines.push(format!("- {}", fix.title));
            for file in &fix.files {
                lines.push(format!("  - file: `{}`", display_path(file)));
            }
        }
    }

    if dry_run {
        if let Some(selected_fixes) = selected_fixes.filter(|fixes| !fixes.is_empty()) {
            lines.push(String::new());
            lines.push("## Planned Changes".to_string());
            for fix in selected_fixes {
                lines.push(format!("- {}", fix.title));
                for file in &fix.files {
                    lines.push(format!("  - file: `{}`", display_path(file)));
                }
            }
        }
    }

    if let Some(preview_report) = preview_report.filter(|report| !report.is_empty()) {
        lines.push(String::new());
        lines.push("## Preview Diffs".to_string());
        lines.push("```diff".to_string());
        lines.extend(preview_report.lines().map(ToOwned::to_owned));
        lines.push("```".to_string());
    }

    lines.push(String::new());
    lines.push(format!(
        "- Post-check: `{}` error, `{}` warnings, `{}` info",
        final_result.summary.blocking_findings,
        final_result.summary.warning_findings,
        final_result.summary.info_findings
    ));

    if final_result.findings.is_empty() {
        lines.push(String::new());
        lines.push("## Remaining Findings".to_string());
        lines.push("- Project is currently clean.".to_string());
    } else {
        lines.push(String::new());
        lines.push("## Remaining Findings".to_string());
        lines.extend(format_findings(final_result));
    }

    lines.join("\n")
}

fn format_top_priorities(result: &AuditResult) -> Vec<String> {
    result
        .findings
        .iter()
        .take(3)
        .enumerate()
        .flat_map(|(index, finding)| {
            let mut lines = vec![format!(
                "{}. **[{}]** {}",
                index + 1,
                severity_label(&finding.severity),
                finding.title
            )];

            if let Some(file) = &finding.file {
                lines.push(format!(
                    "   - file: `{}`",
                    format_relative_file(&result.root_dir, file)
                ));
            }

            if !finding.hint.is_empty() {
                lines.push(format!("   - next: {}", finding.hint));
            } else if !finding.detail.is_empty() {
                lines.push(format!("   - next: {}", finding.detail));
            }

            lines
        })
        .collect()
}

fn format_findings(result: &AuditResult) -> Vec<String> {
    let mut lines = Vec::new();

    for finding in &result.findings {
        lines.push(format!(
            "- **[{}]** {}",
            severity_label(&finding.severity),
            finding.title
        ));

        if let Some(file) = &finding.file {
            lines.push(format!(
                "  - file: `{}`",
                format_relative_file(&result.root_dir, file)
            ));
        }

        if !finding.detail.is_empty() {
            lines.push(format!("  - detail: {}", finding.detail));
        }

        if !finding.hint.is_empty() {
            lines.push(format!("  - hint: {}", finding.hint));
        }
    }

    lines
}

fn describe_structure(structure: &StructureReport) -> String {
    let repo_type = if structure.is_monorepo {
        "monorepo"
    } else {
        "single package"
    };

    format!(
        "{repo_type}, {} package(s), {} config file(s), {} env folder(s)",
        structure.package_count, structure.config_files, structure.env_directories
    )
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

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
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

    use maximus_core::{AuditResult, AuditSummary, Finding, Severity, StructureReport};

    use super::{format_audit_report, format_doctor_report, format_fix_result};

    #[test]
    fn audit_markdown_uses_sectioned_layout() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let report = format_audit_report(&result);

        assert!(report.contains("# Maximus audit"));
        assert!(report.contains("## Structure"));
        assert!(report.contains("## Findings"));
        assert!(report.contains("- Target: `/tmp/project`"));
    }

    #[test]
    fn doctor_markdown_lists_top_priorities() {
        let root_dir = PathBuf::from("/tmp/project");
        let result = AuditResult {
            root_dir: root_dir.clone(),
            summary: AuditSummary {
                status: "blocking issues".to_string(),
                total_findings: 4,
                blocking_findings: 1,
                warning_findings: 1,
                info_findings: 2,
                fixable_findings: 1,
                fixes_available: 1,
                config_files: 2,
                package_count: 1,
                env_directories: 1,
            },
            structure: StructureReport {
                is_monorepo: false,
                package_count: 1,
                env_directories: 1,
                config_files: 2,
                recommendations: Vec::new(),
            },
            findings: vec![
                Finding {
                    id: "err-1".to_string(),
                    severity: Severity::Error,
                    category: "tsconfig".to_string(),
                    title: "First error".to_string(),
                    detail: "first detail".to_string(),
                    file: Some(root_dir.join("tsconfig.json")),
                    hint: "fix the first issue".to_string(),
                    fixable: false,
                    fix_ids: Vec::new(),
                },
                Finding {
                    id: "warn-1".to_string(),
                    severity: Severity::Warn,
                    category: "env".to_string(),
                    title: "Second warning".to_string(),
                    detail: "second detail".to_string(),
                    file: Some(root_dir.join(".env")),
                    hint: "run maximus fix".to_string(),
                    fixable: true,
                    fix_ids: vec!["env-create-example".to_string()],
                },
                Finding {
                    id: "info-1".to_string(),
                    severity: Severity::Info,
                    category: "tsconfig".to_string(),
                    title: "Third note".to_string(),
                    detail: "third detail".to_string(),
                    file: Some(root_dir.join("packages/app/tsconfig.json")),
                    hint: String::new(),
                    fixable: false,
                    fix_ids: Vec::new(),
                },
            ],
            fixes: Vec::new(),
        };

        let report = format_doctor_report(&result);

        assert!(report.contains("## Top Priorities"));
        assert!(report.contains("1. **[error]** First error"));
        assert!(report.contains("   - file: `tsconfig.json`"));
    }

    #[test]
    fn fix_markdown_wraps_preview_diff_blocks() {
        let root_dir = PathBuf::from("/tmp/project");
        let result = sample_result(root_dir.clone());
        let report = format_fix_result(
            true,
            &root_dir,
            &result,
            &[],
            &result,
            None,
            Some("--- /dev/null\n+++ .env.example\n@@ -0,0 +1,1 @@\n+API_URL="),
        );

        assert!(report.contains("## Preview Diffs"));
        assert!(report.contains("```diff"));
        assert!(report.contains("+API_URL="));
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
