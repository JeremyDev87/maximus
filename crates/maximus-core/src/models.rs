use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileKind {
    Package,
    Tsconfig,
    Eslint,
    Prettier,
    Vite,
    Jest,
    Next,
    Env,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub kind: FileKind,
    pub name: String,
    pub path: PathBuf,
    pub dir: PathBuf,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDirectory {
    pub dir: PathBuf,
    pub relative_dir: String,
    pub files: Vec<ProjectFile>,
    pub files_by_kind: IndexMap<FileKind, Vec<ProjectFile>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub root_dir: PathBuf,
    pub files: Vec<ProjectFile>,
    pub directories: Vec<ProjectDirectory>,
    pub files_by_kind: IndexMap<FileKind, Vec<ProjectFile>>,
    pub package_files: Vec<ProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditContext {
    pub root_dir: PathBuf,
    pub runtime: RuntimeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub baseline: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub file: Option<PathBuf>,
    pub hint: String,
    pub fixable: bool,
    pub fix_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixPlan {
    pub id: String,
    pub title: String,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureReport {
    pub is_monorepo: bool,
    pub package_count: usize,
    pub env_directories: usize,
    pub config_files: usize,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub status: String,
    pub total_findings: usize,
    pub blocking_findings: usize,
    pub warning_findings: usize,
    pub info_findings: usize,
    pub fixable_findings: usize,
    pub fixes_available: usize,
    pub config_files: usize,
    pub package_count: usize,
    pub env_directories: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditResult {
    pub root_dir: PathBuf,
    pub summary: AuditSummary,
    pub structure: StructureReport,
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
}
