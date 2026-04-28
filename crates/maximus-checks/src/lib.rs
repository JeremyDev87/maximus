//! Base check implementations for the Maximus Rust rewrite.

mod check_outcome;
mod config_duplicates;
mod editorconfig_prettier;
mod env;
mod eslint_prettier;
mod jsx_config;
pub mod lockfiles;
mod module_system;
mod monorepo_tsconfig;
pub mod package_entrypoints;
pub mod registry;
pub mod structure;
mod test_runner_config;
mod tsconfig;
mod vite_tsconfig_alias;
mod workspace_config;

pub use check_outcome::CheckOutcome;
pub use config_duplicates::run_config_duplicate_check;
pub use editorconfig_prettier::run_editorconfig_prettier_check;
pub use env::{
    render_created_env_example, render_created_env_example_with_sources, render_synced_env_example,
    render_synced_env_example_with_sources, run_env_check, run_env_check_with_options,
    EnvCheckOptions,
};
pub use eslint_prettier::run_eslint_prettier_check;
pub use jsx_config::run_jsx_config_check;
pub use module_system::run_module_system_check;
pub use monorepo_tsconfig::run_monorepo_tsconfig_check;
pub use registry::{
    audit_project, audit_project_with_config, audit_project_with_config_root, registered_check_ids,
    run_env_check_with_config_root_and_options, run_registered_checks,
    run_registered_checks_with_config, run_registered_checks_with_config_root,
    run_registered_checks_with_filters, AuditedProject,
};
pub use structure::build_structure_report;
pub use test_runner_config::run_test_runner_config_check;
pub use tsconfig::run_tsconfig_check;
pub use vite_tsconfig_alias::run_vite_tsconfig_alias_check;
pub use workspace_config::run_workspace_config_check;
