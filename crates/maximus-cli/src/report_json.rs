#![cfg_attr(not(test), allow(dead_code))]

use std::path::{Path, PathBuf};

use maximus_core::{
    serialize_audit_result, AppliedFix, AuditResult, PreviewedFix, SerializableAuditResult,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableFixOutput {
    dry_run: bool,
    target_dir: PathBuf,
    initial: SerializableAuditResult,
    applied: Vec<SerializableAppliedFix>,
    #[serde(rename = "final")]
    final_result: SerializableAuditResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<Vec<SerializablePreviewedFix>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableAppliedFix {
    id: String,
    title: String,
    files: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializablePreviewedFix {
    id: String,
    title: String,
    files: Vec<PathBuf>,
    diffs: Vec<SerializablePreviewedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializablePreviewedFile {
    path: PathBuf,
    existed_before: bool,
    before: String,
    after: String,
}

pub fn render_audit_result(result: &AuditResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&serialize_audit_result(result))
}

pub fn render_fix_result(
    dry_run: bool,
    target_dir: &Path,
    initial: &AuditResult,
    applied: &[AppliedFix],
    final_result: &AuditResult,
    preview: Option<&[PreviewedFix]>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&SerializableFixOutput {
        dry_run,
        target_dir: target_dir.to_path_buf(),
        initial: serialize_audit_result(initial),
        applied: applied
            .iter()
            .map(|fix| SerializableAppliedFix {
                id: fix.id.clone(),
                title: fix.title.clone(),
                files: fix.files.clone(),
                outcome: Some(fix.outcome.clone()),
            })
            .collect(),
        final_result: serialize_audit_result(final_result),
        preview: preview.map(|previews| {
            previews
                .iter()
                .map(|preview| SerializablePreviewedFix {
                    id: preview.id.clone(),
                    title: preview.title.clone(),
                    files: preview.files.clone(),
                    diffs: preview
                        .previews
                        .iter()
                        .map(|file| SerializablePreviewedFile {
                            path: file.path.clone(),
                            existed_before: file.existed_before,
                            before: file.before.clone(),
                            after: file.after.clone(),
                        })
                        .collect(),
                })
                .collect()
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use maximus_core::{AppliedFix, AuditResult, AuditSummary, PreviewedFix, StructureReport};
    use serde_json::Value;

    use super::{render_audit_result, render_fix_result};

    #[test]
    fn audit_json_uses_current_camel_case_shape() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_audit_result(&result).expect("audit json should render");
        let value: Value = serde_json::from_str(&json).expect("audit json should parse");

        assert_eq!(value["schemaVersion"], "1");
        assert_eq!(value["generator"], "maximus");
        assert_eq!(value["rootDir"], "/tmp/project");
        assert_eq!(value["summary"]["blockingFindings"], 0);
        assert_eq!(value["summary"]["suppressedByConfig"], 0);
        assert_eq!(value["structure"]["configFiles"], 1);
        assert!(value["findings"].as_array().is_some());
    }

    #[test]
    fn fix_json_keeps_js_top_level_keys() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_fix_result(
            true,
            PathBuf::from("/tmp/project").as_path(),
            &result,
            &[],
            &result,
            None,
        )
        .expect("fix json should render");
        let value: Value = serde_json::from_str(&json).expect("fix json should parse");

        assert_eq!(value["dryRun"], true);
        assert_eq!(value["targetDir"], "/tmp/project");
        assert!(value.get("initial").is_some());
        assert!(value.get("applied").is_some());
        assert!(value.get("final").is_some());
        assert_eq!(value["initial"]["schemaVersion"], "1");
        assert_eq!(value["initial"]["generator"], "maximus");
        assert_eq!(value["final"]["schemaVersion"], "1");
        assert_eq!(value["final"]["generator"], "maximus");
    }

    #[test]
    fn fix_json_keeps_applied_outcome_when_present() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_fix_result(
            false,
            PathBuf::from("/tmp/project").as_path(),
            &result,
            &[AppliedFix {
                id: "env-example:create:/tmp/project".to_string(),
                title: "Create .env.example".to_string(),
                files: vec![PathBuf::from("/tmp/project/.env.example")],
                outcome: "created".to_string(),
            }],
            &result,
            None,
        )
        .expect("fix json should render");
        let value: Value = serde_json::from_str(&json).expect("fix json should parse");

        assert_eq!(value["applied"][0]["outcome"], "created");
    }

    #[test]
    fn fix_json_includes_preview_when_requested() {
        let result = sample_result(PathBuf::from("/tmp/project"));
        let json = render_fix_result(
            true,
            PathBuf::from("/tmp/project").as_path(),
            &result,
            &[],
            &result,
            Some(&[PreviewedFix {
                id: "env-example:create:/tmp/project".to_string(),
                title: "Create .env.example".to_string(),
                files: vec![PathBuf::from("/tmp/project/.env.example")],
                previews: vec![maximus_core::FixFilePreview {
                    path: PathBuf::from("/tmp/project/.env.example"),
                    existed_before: false,
                    before: String::new(),
                    after: "API_URL=\n".to_string(),
                }],
            }]),
        )
        .expect("fix json should render");
        let value: Value = serde_json::from_str(&json).expect("fix json should parse");

        assert_eq!(value["preview"][0]["id"], "env-example:create:/tmp/project");
        assert_eq!(value["preview"][0]["diffs"][0]["after"], "API_URL=\n");
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
