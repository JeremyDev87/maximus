//! Base check implementations for the Maximus Rust rewrite.

mod check_outcome;
mod config_duplicates;
mod env;
mod eslint_prettier;
pub mod lockfiles;
pub mod structure;
mod tsconfig;
pub mod registry;

pub use check_outcome::CheckOutcome;
pub use config_duplicates::run_config_duplicate_check;
pub use env::{render_created_env_example, render_synced_env_example, run_env_check};
pub use eslint_prettier::run_eslint_prettier_check;
pub use registry::{audit_project, run_registered_checks, AuditedProject};
pub use structure::build_structure_report;
pub use tsconfig::run_tsconfig_check;
