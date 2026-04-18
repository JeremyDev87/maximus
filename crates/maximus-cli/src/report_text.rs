#![cfg_attr(not(test), allow(dead_code))]

use std::path::Path;

use maximus_core::{AuditResult, StructureReport};

use crate::AppliedFix;

pub fn format_help() -> String {
    [
        "Maximus",
        "",
        "Bring order to chaotic configs.",
        "",
        "Usage",
        "  maximus audit [path] [--json]",
        "  maximus doctor [path] [--json]",
        "  maximus fix [path] [--dry-run] [--json]",
        "  maximus help",
    ]
    .join("\n")
}

pub fn format_audit_report(result: &AuditResult) -> String {
    let mut lines = Vec::new();

    lines.push("Maximus audit".to_string());
    lines.push(format!("Target: {}", display_path(&result.root_dir)));
    lines.push(String::new());
    lines.push(format!("Status: {}", result.summary.status));
    lines.push(format!(
        "Findings: {} error, {} warnings, {} info",
        result.summary.blocking_findings, result.summary.warning_findings, result.summary.info_findings
    ));
    lines.push(format!(
        "Fixes available: {}",
        result.summary.fixes_available
    ));
    lines.push(String::new());
    lines.push(format!("Structure: {}", describe_structure(&result.structure)));

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("No config drift detected.".to_string());
    } else {
        lines.push(String::new());
        lines.push("Findings".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("Recommendations".to_string());
        for recommendation in &result.structure.recommendations {
            lines.push(format!("- {recommendation}"));
        }
    }

    lines.join("\n")
}

pub fn format_doctor_report(result: &AuditResult) -> String {
    let mut lines = Vec::new();
    let manual_findings = result
        .findings
        .iter()
        .filter(|finding| !finding.fixable)
        .count();
    let fixable_findings = result.findings.iter().filter(|finding| finding.fixable).count();

    lines.push("Maximus doctor".to_string());
    lines.push(format!("Target: {}", display_path(&result.root_dir)));
    lines.push(String::new());
    lines.push(format!("Diagnosis: {}", result.summary.status));
    lines.push(format!(
        "Project shape: {}",
        describe_structure(&result.structure)
    ));
    lines.push(String::new());
    lines.push("Prescription".to_string());

    if fixable_findings > 0 {
        lines.push(format!(
            "- Run \"maximus fix\" to apply {} safe fix(es).",
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
        lines.push("No config drift detected.".to_string());
    } else {
        lines.push(String::new());
        lines.push("Findings".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("Recommended structure".to_string());
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
) -> String {
    let mut lines = Vec::new();
    let result = if dry_run { initial } else { final_result };

    lines.push("Maximus fix".to_string());
    lines.push(format!("Target: {}", display_path(target_dir)));
    lines.push(String::new());

    if dry_run {
        lines.push(format!(
            "Dry run: {} safe fix(es) available.",
            initial.summary.fixes_available
        ));
    } else {
        lines.push(format!("Applied: {} fix(es).", applied.len()));
    }

    if !applied.is_empty() {
        lines.push(String::new());
        lines.push("Changes".to_string());
        for fix in applied {
            lines.push(format!("- {}", fix.title));
            for file in &fix.files {
                lines.push(format!("  file: {}", display_path(file)));
            }
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Post-check: {} error, {} warnings, {} info",
        result.summary.blocking_findings, result.summary.warning_findings, result.summary.info_findings
    ));

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("Project is currently clean.".to_string());
    } else {
        lines.push(String::new());
        lines.push("Remaining findings".to_string());
        lines.extend(format_findings(result));
    }

    lines.join("\n")
}

fn format_findings(result: &AuditResult) -> Vec<String> {
    let mut lines = Vec::new();

    for finding in &result.findings {
        lines.push(format!("- [{}] {}", severity_label(&finding.severity), finding.title));

        if let Some(file) = &finding.file {
            lines.push(format!(
                "  file: {}",
                format_relative_file(&result.root_dir, file)
            ));
        }

        if !finding.detail.is_empty() {
            lines.push(format!("  detail: {}", finding.detail));
        }

        if !finding.hint.is_empty() {
            lines.push(format!("  hint: {}", finding.hint));
        }
    }

    lines
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

fn severity_label(severity: &maximus_core::Severity) -> &'static str {
    match severity {
        maximus_core::Severity::Error => "error",
        maximus_core::Severity::Warn => "warn",
        maximus_core::Severity::Info => "info",
    }
}

fn display_path(path: &Path) -> String {
    path_string(path)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use maximus_core::{AuditResult, AuditSummary, StructureReport};

    use crate::AppliedFix;

    use super::{
        format_audit_report, format_doctor_report, format_fix_result, format_help,
        format_relative_file,
    };

    #[test]
    fn help_text_matches_current_js_usage() {
        assert_eq!(
            format_help(),
            [
                "Maximus",
                "",
                "Bring order to chaotic configs.",
                "",
                "Usage",
                "  maximus audit [path] [--json]",
                "  maximus doctor [path] [--json]",
                "  maximus fix [path] [--dry-run] [--json]",
                "  maximus help",
            ]
            .join("\n")
        );
    }

    #[test]
    fn text_renderers_match_clean_project_shape() {
        let root_dir = PathBuf::from("/tmp/project");
        let result = sample_result(root_dir.clone());

        assert_eq!(
            format_audit_report(&result),
            [
                "Maximus audit",
                "Target: /tmp/project",
                "",
                "Status: clean",
                "Findings: 0 error, 0 warnings, 0 info",
                "Fixes available: 0",
                "",
                "Structure: single package, 1 package(s), 1 config file(s), 0 env folder(s)",
                "",
                "No config drift detected.",
                "",
                "Recommendations",
                "- Current config surface looks healthy. Keep shared rules centralized as the repo grows.",
            ]
            .join("\n")
        );

        assert_eq!(
            format_doctor_report(&result),
            [
                "Maximus doctor",
                "Target: /tmp/project",
                "",
                "Diagnosis: clean",
                "Project shape: single package, 1 package(s), 1 config file(s), 0 env folder(s)",
                "",
                "Prescription",
                "- No automatic fixes are currently available.",
                "- No manual follow-up is required right now.",
                "",
                "No config drift detected.",
                "",
                "Recommended structure",
                "- Current config surface looks healthy. Keep shared rules centralized as the repo grows.",
            ]
            .join("\n")
        );

        assert_eq!(
            format_fix_result(true, &root_dir, &result, &Vec::<AppliedFix>::new(), &result),
            [
                "Maximus fix",
                "Target: /tmp/project",
                "",
                "Dry run: 0 safe fix(es) available.",
                "",
                "Post-check: 0 error, 0 warnings, 0 info",
                "",
                "Project is currently clean.",
            ]
            .join("\n")
        );
    }

    #[test]
    fn relative_file_format_matches_js_like_root_and_parent_cases() {
        let root_dir = PathBuf::from("/tmp/project");

        assert_eq!(format_relative_file(&root_dir, &root_dir), ".");
        assert_eq!(
            format_relative_file(&root_dir, &root_dir.join("src/index.ts")),
            "src/index.ts"
        );
        assert_eq!(
            format_relative_file(&root_dir, Path::new("/tmp/other/file.ts")),
            "../other/file.ts"
        );
    }

    #[cfg(windows)]
    #[test]
    fn relative_file_format_falls_back_to_target_path_for_cross_drive_windows_paths() {
        let root_dir = Path::new(r"C:\repo");
        let file_path = Path::new(r"D:\other\file.ts");

        assert_eq!(
            format_relative_file(root_dir, file_path),
            r"D:\other\file.ts"
        );
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
