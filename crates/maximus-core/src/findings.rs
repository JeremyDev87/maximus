use serde::Serialize;

use crate::models::{AuditResult, AuditSummary, Finding, FixPlan, Severity, StructureReport};
use crate::text_order::locale_compare_like;

pub const JSON_GENERATOR: &str = "maximus";
pub const JSON_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingInput {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub detail: Option<String>,
    pub file: Option<std::path::PathBuf>,
    pub fix_ids: Vec<String>,
    pub fixable: bool,
    pub hint: Option<String>,
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableAuditResult {
    pub schema_version: &'static str,
    pub generator: &'static str,
    pub root_dir: std::path::PathBuf,
    pub summary: AuditSummary,
    pub structure: StructureReport,
    pub findings: Vec<SerializableFinding>,
    pub fixes: Vec<SerializableFixPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableFinding {
    pub id: String,
    pub severity: Severity,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub file: Option<std::path::PathBuf>,
    pub hint: String,
    pub fixable: bool,
    pub fix_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerializableFixPlan {
    pub id: String,
    pub title: String,
    pub files: Vec<std::path::PathBuf>,
}

pub fn make_finding(input: FindingInput) -> Finding {
    Finding {
        id: input.id,
        severity: input.severity.unwrap_or(Severity::Warn),
        category: input.category.unwrap_or_else(|| "general".to_string()),
        title: input.title,
        detail: input.detail.unwrap_or_default(),
        file: input.file,
        hint: input.hint.unwrap_or_default(),
        fixable: input.fixable,
        fix_ids: input.fix_ids,
    }
}

pub fn sort_findings(findings: &[Finding]) -> Vec<Finding> {
    let mut sorted = findings.to_vec();
    sorted.sort_by(|left, right| {
        let left_file = left
            .file
            .as_ref()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        let right_file = right
            .file
            .as_ref()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();

        severity_rank(&left.severity)
            .cmp(&severity_rank(&right.severity))
            .then_with(|| locale_compare_like(&left_file, &right_file))
            .then_with(|| locale_compare_like(&left.title, &right.title))
    });
    sorted
}

pub fn unique_fixes(fixes: &[FixPlan]) -> Vec<FixPlan> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();

    for fix in fixes {
        if seen.insert(fix.id.clone()) {
            unique.push(fix.clone());
        }
    }

    unique
}

pub fn summarize_findings(
    findings: &[Finding],
    fixes: &[FixPlan],
    structure: &StructureReport,
) -> AuditSummary {
    summarize_findings_with_suppressed_by_config(findings, fixes, structure, 0)
}

pub fn summarize_findings_with_suppressed_by_config(
    findings: &[Finding],
    fixes: &[FixPlan],
    structure: &StructureReport,
    suppressed_by_config: usize,
) -> AuditSummary {
    let blocking_findings = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .count();
    let warning_findings = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Warn)
        .count();
    let info_findings = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Info)
        .count();
    let fixable_findings = findings.iter().filter(|finding| finding.fixable).count();

    let status = if blocking_findings > 0 {
        "blocking issues"
    } else if warning_findings > 0 {
        "attention needed"
    } else {
        "clean"
    };

    AuditSummary {
        status: status.to_string(),
        total_findings: findings.len(),
        blocking_findings,
        warning_findings,
        info_findings,
        fixable_findings,
        fixes_available: fixes.len(),
        suppressed_by_config,
        config_files: structure.config_files,
        package_count: structure.package_count,
        env_directories: structure.env_directories,
    }
}

pub fn serialize_audit_result(result: &AuditResult) -> SerializableAuditResult {
    SerializableAuditResult {
        schema_version: JSON_SCHEMA_VERSION,
        generator: JSON_GENERATOR,
        root_dir: result.root_dir.clone(),
        summary: result.summary.clone(),
        structure: result.structure.clone(),
        findings: result
            .findings
            .iter()
            .map(|finding| SerializableFinding {
                id: finding.id.clone(),
                severity: finding.severity.clone(),
                category: finding.category.clone(),
                title: finding.title.clone(),
                detail: finding.detail.clone(),
                file: finding.file.clone(),
                hint: finding.hint.clone(),
                fixable: finding.fixable,
                fix_ids: finding.fix_ids.clone(),
            })
            .collect(),
        fixes: result
            .fixes
            .iter()
            .map(|fix| SerializableFixPlan {
                id: fix.id.clone(),
                title: fix.title.clone(),
                files: fix.files.clone(),
            })
            .collect(),
    }
}

fn severity_rank(severity: &Severity) -> usize {
    match severity {
        Severity::Error => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    }
}
