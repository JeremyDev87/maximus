//! Base check implementations for the Maximus Rust rewrite.

mod check_outcome;
mod config_duplicates;
mod env;
mod eslint_prettier;
pub mod lockfiles;
pub mod package_entrypoints;
pub mod registry;
pub mod structure;
mod tsconfig;

pub use check_outcome::CheckOutcome;
pub use config_duplicates::run_config_duplicate_check;
pub use env::{render_created_env_example, render_synced_env_example, run_env_check};
pub use eslint_prettier::run_eslint_prettier_check;
pub use registry::{
    audit_project, audit_project_with_config, audit_project_with_config_root, registered_check_ids,
    run_registered_checks, run_registered_checks_with_config,
    run_registered_checks_with_config_root, run_registered_checks_with_filters, AuditedProject,
};
pub use structure::build_structure_report;
pub use tsconfig::run_tsconfig_check;
