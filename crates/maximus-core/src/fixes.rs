use std::io;
use std::path::{Path, PathBuf};

use crate::env_parser::render_env_template;
use crate::fs::write_text;
use crate::models::FixPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixOperation {
    CreateEnvExample {
        output_path: PathBuf,
        keys: Vec<String>,
    },
    SyncEnvExample {
        example_path: PathBuf,
        existing_text: String,
        missing_keys: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFix {
    pub public: FixPlan,
    pub operation: FixOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedFix {
    pub id: String,
    pub title: String,
    pub files: Vec<PathBuf>,
    pub outcome: String,
}

pub fn plan_create_env_example(
    root_dir: &Path,
    directory: &Path,
    keys: &[String],
) -> PlannedFix {
    let output_path = directory.join(".env.example");

    PlannedFix {
        public: FixPlan {
            id: format!("env-example:create:{}", directory.to_string_lossy()),
            title: format!("Create {}", relative_or_fallback(root_dir, &output_path, ".env.example")),
            files: vec![output_path.clone()],
        },
        operation: FixOperation::CreateEnvExample {
            output_path,
            keys: keys.to_vec(),
        },
    }
}

pub fn plan_sync_env_example(
    root_dir: &Path,
    example_path: &Path,
    existing_text: &str,
    missing_keys: &[String],
) -> PlannedFix {
    let file_name = example_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".env.example".to_string());

    PlannedFix {
        public: FixPlan {
            id: format!(
                "env-example:sync:{}",
                example_path
                    .parent()
                    .unwrap_or(example_path)
                    .to_string_lossy()
            ),
            title: format!(
                "Append missing keys to {}",
                relative_or_fallback(root_dir, example_path, &file_name)
            ),
            files: vec![example_path.to_path_buf()],
        },
        operation: FixOperation::SyncEnvExample {
            example_path: example_path.to_path_buf(),
            existing_text: existing_text.to_string(),
            missing_keys: missing_keys.to_vec(),
        },
    }
}

pub fn apply_fix(fix: &PlannedFix) -> io::Result<AppliedFix> {
    match &fix.operation {
        FixOperation::CreateEnvExample { output_path, keys } => {
            write_text(output_path, &render_env_template(keys.iter().map(|key| key.as_str())))?;
            Ok(applied_fix_from_public(&fix.public, "created"))
        }
        FixOperation::SyncEnvExample {
            example_path,
            existing_text,
            missing_keys,
        } => {
            let prefix = if existing_text.ends_with('\n') || existing_text.is_empty() {
                ""
            } else {
                "\n"
            };
            let addition = render_env_template(missing_keys.iter().map(|key| key.as_str()));

            write_text(
                example_path,
                &format!("{existing_text}{prefix}{addition}"),
            )?;

            Ok(applied_fix_from_public(&fix.public, "updated"))
        }
    }
}

pub fn apply_fixes(fixes: &[PlannedFix]) -> io::Result<Vec<AppliedFix>> {
    fixes.iter().map(apply_fix).collect()
}

fn relative_or_fallback(root_dir: &Path, target_path: &Path, fallback: &str) -> String {
    target_path
        .strip_prefix(root_dir)
        .ok()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn applied_fix_from_public(fix: &FixPlan, outcome: &str) -> AppliedFix {
    AppliedFix {
        id: fix.id.clone(),
        title: fix.title.clone(),
        files: fix.files.clone(),
        outcome: outcome.to_string(),
    }
}
