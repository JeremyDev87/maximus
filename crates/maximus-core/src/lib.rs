//! Pure data-model and parsing helpers for the Maximus Rust rewrite.

pub mod baseline;
pub mod config;
pub mod discover;
pub mod env_parser;
pub mod fixes;
pub mod findings;
pub mod fs;
pub mod jsonc;
pub mod models;
mod text_order;

pub use discover::{discover_project, find_nearest_package_file, get_directories, get_files};
pub use config::{
    find_maximus_config_path, load_maximus_config, CheckFilterConfig, ConfigSeverity,
    FailOnLevel, LoadedConfig, LoadConfigError, MaximusConfig, ReportConfig,
};
pub use env_parser::{
    is_concrete_env_file_name, is_template_env_file_name, looks_like_secret, parse_env,
    render_env_template, EnvDuplicate, EnvEntry, InvalidEnvLine, ParsedEnv,
};
pub use fixes::{
    apply_fix, apply_fixes, plan_create_env_example, plan_sync_env_example, AppliedFix,
    FixOperation, PlannedFix,
};
pub use findings::{
    make_finding, serialize_audit_result, sort_findings, summarize_findings, unique_fixes,
    FindingInput, SerializableAuditResult, SerializableFinding,
};
pub use fs::{path_exists, read_text_if_exists, write_text};
pub use jsonc::{parse_jsonc, ParseJsoncError};
pub use models::{
    AuditContext, AuditResult, AuditSummary, BaselineEntry, CheckId, FileKind, Finding, FixPlan,
    ProjectDirectory, ProjectFile, ProjectSnapshot, RuntimeConfig, Severity, StructureReport,
};
