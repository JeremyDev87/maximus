#![cfg_attr(not(test), allow(dead_code))]

use std::path::Path;

use maximus_core::{AppliedFix, AuditResult, FixPlan};

use crate::report_ko as ko;

pub fn format_help() -> String {
    [
        "Maximus",
        "",
        "혼란스러운 설정을 정리합니다.",
        "",
        "사용법",
        "  maximus audit [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
        "  maximus doctor [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
        "  maximus fix [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--dry-run] [--diff] [--env-source-comments] [--fix-id <id>] [--fix-prefix <prefix>] [--format <format>] [--json] [--output <path>]",
        "  maximus help",
    ]
    .join("\n")
}

pub fn format_audit_report(result: &AuditResult) -> String {
    let mut lines = Vec::new();

    lines.push("Maximus audit".to_string());
    lines.push(format!("대상: {}", display_path(&result.root_dir)));
    lines.push(String::new());
    lines.push(format!(
        "상태: {}",
        ko::status_label(&result.summary.status)
    ));
    lines.push(format!(
        "발견 항목: 오류 {}개, 경고 {}개, 정보 {}개",
        result.summary.blocking_findings,
        result.summary.warning_findings,
        result.summary.info_findings
    ));
    push_suppression_summary(&mut lines, result.summary.suppressed_by_config);
    lines.push(format!(
        "적용 가능한 수정: {}개",
        result.summary.fixes_available
    ));
    lines.push(String::new());
    lines.push(format!(
        "구조: {}",
        ko::describe_structure(&result.structure)
    ));

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("설정 차이가 감지되지 않았습니다.".to_string());
    } else {
        lines.push(String::new());
        lines.push("발견 항목".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("권장 사항".to_string());
        for recommendation in &result.structure.recommendations {
            lines.push(format!("- {}", ko::message(recommendation)));
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
    let fixable_findings = result
        .findings
        .iter()
        .filter(|finding| finding.fixable)
        .count();

    lines.push("Maximus doctor".to_string());
    lines.push(format!("대상: {}", display_path(&result.root_dir)));
    lines.push(String::new());
    lines.push(format!(
        "진단: {}",
        ko::status_label(&result.summary.status)
    ));
    push_suppression_summary(&mut lines, result.summary.suppressed_by_config);
    lines.push(format!(
        "프로젝트 구조: {}",
        ko::describe_structure(&result.structure)
    ));
    lines.push(String::new());
    lines.push("처방".to_string());

    if fixable_findings > 0 {
        lines.push(format!(
            "- 안전한 수정 {}개를 적용하려면 \"maximus fix\"를 실행하세요.",
            result.summary.fixes_available
        ));
    } else {
        lines.push("- 현재 적용 가능한 자동 수정이 없습니다.".to_string());
    }

    if manual_findings > 0 {
        lines.push(format!(
            "- 아래 우선순위에 따라 수동 확인 항목 {manual_findings}개를 검토하세요."
        ));
    } else {
        lines.push("- 지금은 수동 후속 조치가 필요하지 않습니다.".to_string());
    }

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("설정 차이가 감지되지 않았습니다.".to_string());
    } else {
        lines.push(String::new());
        lines.push("상위 3개 우선순위".to_string());
        lines.extend(format_top_priorities(result));
        lines.push(String::new());
        lines.push("발견 항목".to_string());
        lines.extend(format_findings(result));
    }

    if !result.structure.recommendations.is_empty() {
        lines.push(String::new());
        lines.push("권장 구조".to_string());
        for recommendation in &result.structure.recommendations {
            lines.push(format!("- {}", ko::message(recommendation)));
        }
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
                "{}. [{}] {}",
                index + 1,
                ko::severity_label(&finding.severity),
                ko::message(&finding.title)
            )];

            if let Some(file) = &finding.file {
                lines.push(format!(
                    "   파일: {}",
                    format_relative_file(&result.root_dir, file)
                ));
            }

            if !finding.hint.is_empty() {
                lines.push(format!("   다음: {}", ko::message(&finding.hint)));
            } else if !finding.detail.is_empty() {
                lines.push(format!("   다음: {}", ko::message(&finding.detail)));
            }

            lines
        })
        .collect()
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
    let mut lines = Vec::new();
    let result = if dry_run { initial } else { final_result };
    let should_show_selected_fixes = selected_fixes.is_some_and(|fixes| !fixes.is_empty());
    let selected_fixes = selected_fixes.unwrap_or(&[]);

    lines.push("Maximus fix".to_string());
    lines.push(format!("대상: {}", display_path(target_dir)));
    lines.push(String::new());

    if dry_run {
        if should_show_selected_fixes {
            lines.push(format!(
                "Dry run: 안전한 수정 {}개가 선택되었습니다.",
                selected_fixes.len()
            ));
        } else {
            lines.push(format!(
                "Dry run: 적용 가능한 안전한 수정 {}개가 있습니다.",
                initial.summary.fixes_available
            ));
        }
    } else {
        lines.push(format!("적용됨: 수정 {}개.", applied.len()));
    }

    if !applied.is_empty() {
        lines.push(String::new());
        lines.push("변경 사항".to_string());
        for fix in applied {
            lines.push(format!("- {}", ko::fix_title(&fix.title)));
            for file in &fix.files {
                lines.push(format!("  파일: {}", display_path(file)));
            }
        }
    }

    if dry_run && should_show_selected_fixes {
        lines.push(String::new());
        lines.push("계획된 변경 사항".to_string());
        for fix in selected_fixes {
            lines.push(format!("- {}", ko::fix_title(&fix.title)));
            for file in &fix.files {
                lines.push(format!("  파일: {}", display_path(file)));
            }
        }
    }

    if let Some(preview_report) = preview_report.filter(|report| !report.is_empty()) {
        lines.push(String::new());
        lines.push("미리보기 diff".to_string());
        lines.extend(preview_report.lines().map(ToString::to_string));
    }

    lines.push(String::new());
    lines.push(format!(
        "사후 점검: 오류 {}개, 경고 {}개, 정보 {}개",
        result.summary.blocking_findings,
        result.summary.warning_findings,
        result.summary.info_findings
    ));
    push_suppression_summary(&mut lines, result.summary.suppressed_by_config);

    if result.findings.is_empty() {
        lines.push(String::new());
        lines.push("현재 프로젝트는 정상입니다.".to_string());
    } else {
        lines.push(String::new());
        lines.push("남은 발견 항목".to_string());
        lines.extend(format_findings(result));
    }

    lines.join("\n")
}

fn format_findings(result: &AuditResult) -> Vec<String> {
    let mut lines = Vec::new();

    for finding in &result.findings {
        lines.push(format!(
            "- [{}] {}",
            ko::severity_label(&finding.severity),
            ko::message(&finding.title)
        ));

        if let Some(file) = &finding.file {
            lines.push(format!(
                "  파일: {}",
                format_relative_file(&result.root_dir, file)
            ));
        }

        if !finding.detail.is_empty() {
            lines.push(format!("  상세: {}", ko::message(&finding.detail)));
        }

        if !finding.hint.is_empty() {
            lines.push(format!("  힌트: {}", ko::message(&finding.hint)));
        }
    }

    lines
}

fn push_suppression_summary(lines: &mut Vec<String>, suppressed_by_config: usize) {
    if suppressed_by_config > 0 {
        lines.push(format!("설정으로 숨김: {suppressed_by_config}개"));
    }
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

    use maximus_core::{AppliedFix, AuditResult, AuditSummary, Finding, Severity, StructureReport};

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
                "혼란스러운 설정을 정리합니다.",
                "",
                "사용법",
                "  maximus audit [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
                "  maximus doctor [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
                "  maximus fix [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--dry-run] [--diff] [--env-source-comments] [--fix-id <id>] [--fix-prefix <prefix>] [--format <format>] [--json] [--output <path>]",
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
                "대상: /tmp/project",
                "",
                "상태: 정상",
                "발견 항목: 오류 0개, 경고 0개, 정보 0개",
                "적용 가능한 수정: 0개",
                "",
                "구조: 단일 패키지, 패키지 1개, 설정 파일 1개, env 폴더 0개",
                "",
                "설정 차이가 감지되지 않았습니다.",
                "",
                "권장 사항",
                "- 현재 설정 표면은 정상입니다. repo가 커져도 shared rule을 중앙에 유지하세요.",
            ]
            .join("\n")
        );

        assert_eq!(
            format_doctor_report(&result),
            [
                "Maximus doctor",
                "대상: /tmp/project",
                "",
                "진단: 정상",
                "프로젝트 구조: 단일 패키지, 패키지 1개, 설정 파일 1개, env 폴더 0개",
                "",
                "처방",
                "- 현재 적용 가능한 자동 수정이 없습니다.",
                "- 지금은 수동 후속 조치가 필요하지 않습니다.",
                "",
                "설정 차이가 감지되지 않았습니다.",
                "",
                "권장 구조",
                "- 현재 설정 표면은 정상입니다. repo가 커져도 shared rule을 중앙에 유지하세요.",
            ]
            .join("\n")
        );

        assert_eq!(
            format_fix_result(
                true,
                &root_dir,
                &result,
                &Vec::<AppliedFix>::new(),
                &result,
                None,
                None,
            ),
            [
                "Maximus fix",
                "대상: /tmp/project",
                "",
                "Dry run: 적용 가능한 안전한 수정 0개가 있습니다.",
                "",
                "사후 점검: 오류 0개, 경고 0개, 정보 0개",
                "",
                "현재 프로젝트는 정상입니다.",
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

    #[test]
    fn doctor_report_lists_only_first_three_priorities() {
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
                suppressed_by_config: 0,
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
                Finding {
                    id: "info-2".to_string(),
                    severity: Severity::Info,
                    category: "general".to_string(),
                    title: "Fourth note".to_string(),
                    detail: "fourth detail".to_string(),
                    file: None,
                    hint: String::new(),
                    fixable: false,
                    fix_ids: Vec::new(),
                },
            ],
            fixes: Vec::new(),
        };

        let report = format_doctor_report(&result);

        assert!(report.contains(
            [
                "상위 3개 우선순위",
                "1. [오류] First error",
                "   파일: tsconfig.json",
                "   다음: fix the first issue",
                "2. [경고] Second warning",
                "   파일: .env",
                "   다음: run maximus fix",
                "3. [정보] Third note",
                "   파일: packages/app/tsconfig.json",
                "   다음: third detail",
            ]
            .join("\n")
            .as_str()
        ));
        assert!(report.contains("- [정보] Fourth note"));
        assert!(!report.contains("4. [정보] Fourth note"));
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
                suppressed_by_config: 0,
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
