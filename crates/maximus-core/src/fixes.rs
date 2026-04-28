use std::io;
use std::path::{Path, PathBuf};

use crate::env_parser::{
    render_env_template_groups, EnvTemplateRenderOptions, EnvTemplateSourceGroup,
};
use crate::fs::{read_text_if_exists, write_text};
use crate::models::FixPlan;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FixSelector {
    pub ids: Vec<String>,
    pub prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixOperation {
    CreateEnvExample {
        output_path: PathBuf,
        groups: Vec<EnvTemplateSourceGroup>,
        render_options: EnvTemplateRenderOptions,
    },
    SyncEnvExample {
        example_path: PathBuf,
        existing_text: String,
        groups: Vec<EnvTemplateSourceGroup>,
        render_options: EnvTemplateRenderOptions,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixFilePreview {
    pub path: PathBuf,
    pub existed_before: bool,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewedFix {
    pub id: String,
    pub title: String,
    pub files: Vec<PathBuf>,
    pub previews: Vec<FixFilePreview>,
}

pub fn plan_create_env_example(root_dir: &Path, directory: &Path, keys: &[String]) -> PlannedFix {
    plan_create_env_example_with_groups(
        root_dir,
        directory,
        vec![EnvTemplateSourceGroup {
            source: None,
            keys: keys.to_vec(),
        }],
        EnvTemplateRenderOptions::default(),
    )
}

pub fn plan_create_env_example_with_groups(
    root_dir: &Path,
    directory: &Path,
    groups: Vec<EnvTemplateSourceGroup>,
    render_options: EnvTemplateRenderOptions,
) -> PlannedFix {
    let output_path = directory.join(".env.example");

    PlannedFix {
        public: FixPlan {
            id: format!("env-example:create:{}", directory.to_string_lossy()),
            title: format!(
                "Create {}",
                relative_or_fallback(root_dir, &output_path, ".env.example")
            ),
            files: vec![output_path.clone()],
        },
        operation: FixOperation::CreateEnvExample {
            output_path,
            groups,
            render_options,
        },
    }
}

pub fn plan_sync_env_example(
    root_dir: &Path,
    example_path: &Path,
    existing_text: &str,
    missing_keys: &[String],
) -> PlannedFix {
    plan_sync_env_example_with_groups(
        root_dir,
        example_path,
        existing_text,
        vec![EnvTemplateSourceGroup {
            source: None,
            keys: missing_keys.to_vec(),
        }],
        EnvTemplateRenderOptions::default(),
    )
}

pub fn plan_sync_env_example_with_groups(
    root_dir: &Path,
    example_path: &Path,
    existing_text: &str,
    groups: Vec<EnvTemplateSourceGroup>,
    render_options: EnvTemplateRenderOptions,
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
            groups,
            render_options,
        },
    }
}

pub fn apply_fix(fix: &PlannedFix) -> io::Result<AppliedFix> {
    match &fix.operation {
        FixOperation::CreateEnvExample {
            output_path,
            groups,
            render_options,
        } => {
            write_text(
                output_path,
                &render_env_template_groups(groups.clone(), render_options),
            )?;
            Ok(applied_fix_from_public(&fix.public, "created"))
        }
        FixOperation::SyncEnvExample {
            example_path,
            existing_text,
            groups,
            render_options,
        } => {
            let prefix = if existing_text.ends_with('\n') || existing_text.is_empty() {
                ""
            } else {
                "\n"
            };
            let addition = render_env_template_groups(groups.clone(), render_options);

            write_text(example_path, &format!("{existing_text}{prefix}{addition}"))?;

            Ok(applied_fix_from_public(&fix.public, "updated"))
        }
    }
}

pub fn apply_fixes(fixes: &[PlannedFix]) -> io::Result<Vec<AppliedFix>> {
    fixes.iter().map(apply_fix).collect()
}

pub fn select_fix_plans(fixes: &[FixPlan], selector: &FixSelector) -> Vec<FixPlan> {
    fixes
        .iter()
        .filter(|fix| selector.matches(&fix.id))
        .cloned()
        .collect()
}

pub fn select_planned_fixes(fixes: &[PlannedFix], selector: &FixSelector) -> Vec<PlannedFix> {
    fixes
        .iter()
        .filter(|fix| selector.matches(&fix.public.id))
        .cloned()
        .collect()
}

pub fn preview_fixes(fixes: &[PlannedFix]) -> io::Result<Vec<PreviewedFix>> {
    fixes.iter().map(preview_fix).collect()
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

impl FixSelector {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.prefixes.is_empty()
    }

    pub fn matches(&self, fix_id: &str) -> bool {
        if self.is_empty() {
            return true;
        }

        self.ids.iter().any(|id| id == fix_id)
            || self
                .prefixes
                .iter()
                .any(|prefix| fix_id.starts_with(prefix))
    }
}

fn preview_fix(fix: &PlannedFix) -> io::Result<PreviewedFix> {
    let previews = match &fix.operation {
        FixOperation::CreateEnvExample {
            output_path,
            groups,
            render_options,
        } => {
            let before = read_text_if_exists(output_path)?.unwrap_or_default();
            let existed_before = output_path.exists();
            let after = render_env_template_groups(groups.clone(), render_options);

            vec![FixFilePreview {
                path: output_path.clone(),
                existed_before,
                before,
                after,
            }]
        }
        FixOperation::SyncEnvExample {
            example_path,
            existing_text,
            groups,
            render_options,
        } => {
            let prefix = if existing_text.ends_with('\n') || existing_text.is_empty() {
                ""
            } else {
                "\n"
            };
            let addition = render_env_template_groups(groups.clone(), render_options);

            vec![FixFilePreview {
                path: example_path.clone(),
                existed_before: true,
                before: existing_text.clone(),
                after: format!("{existing_text}{prefix}{addition}"),
            }]
        }
    };

    Ok(PreviewedFix {
        id: fix.public.id.clone(),
        title: fix.public.title.clone(),
        files: fix.public.files.clone(),
        previews,
    })
}
