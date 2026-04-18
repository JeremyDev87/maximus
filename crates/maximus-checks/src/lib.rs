//! Base check implementations for the Maximus Rust rewrite.

mod config_duplicates;
mod eslint_prettier;
pub mod registry;

pub use config_duplicates::run_config_duplicate_check;
pub use eslint_prettier::run_eslint_prettier_check;
pub use registry::{run_registered_checks, CheckOutcome};
