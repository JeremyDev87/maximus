mod args;
mod exit_codes;
mod report_diff;
mod report_json;
mod report_text;

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io;
use std::process;

use maximus_checks::audit_project;
use maximus_core::{
    apply_fixes, preview_fixes, select_fix_plans, select_planned_fixes, AuditResult, FixPlan,
    FixSelector,
};

use crate::args::{parse_args, ArgsError, Flags};

#[derive(Debug)]
enum CliError {
    Args(ArgsError),
    Io(io::Error),
    Json(serde_json::Error),
    InvalidArguments(String),
    UnknownCommand(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Args(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::InvalidArguments(message) => write!(f, "{message}"),
            Self::UnknownCommand(command) => {
                write!(f, "Unknown command \"{command}\". Run \"maximus help\" for usage.")
            }
        }
    }
}

impl Error for CliError {}

impl From<ArgsError> for CliError {
    fn from(value: ArgsError) -> Self {
        Self::Args(value)
    }
}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

fn main() {
    let exit_code = match run_cli(env::args_os().skip(1)) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("Maximus failed: {error}");
            exit_codes::FAILURE
        }
    };

    process::exit(exit_code);
}

fn run_cli<I, S>(argv: I) -> Result<i32, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString>,
{
    let parsed = parse_args(argv)?;
    validate_command_flags(parsed.command.as_deref(), &parsed.flags)?;

    if parsed.command.is_none()
        || parsed.command.as_deref() == Some("help")
        || parsed.flags.help
    {
        println!("{}", report_text::format_help());
        return Ok(exit_codes::SUCCESS);
    }

    let target_dir = resolve_target_dir(parsed.args.first().map(|value| value.as_os_str()))?;

    match parsed.command.as_deref() {
        Some("audit") => run_audit_command(&target_dir, &parsed.flags),
        Some("doctor") => run_doctor_command(&target_dir, &parsed.flags),
        Some("fix") => run_fix_command(&target_dir, &parsed.flags),
        Some(command) => Err(CliError::UnknownCommand(command.to_string())),
        None => Ok(exit_codes::SUCCESS),
    }
}

fn run_audit_command(target_dir: &std::path::Path, flags: &Flags) -> Result<i32, CliError> {
    let audited = audit_project(target_dir)?;

    if flags.json {
        println!("{}", report_json::render_audit_result(&audited.result)?);
    } else {
        println!("{}", report_text::format_audit_report(&audited.result));
    }

    Ok(exit_codes::audit_exit_code(&audited.result.summary))
}

fn run_doctor_command(target_dir: &std::path::Path, flags: &Flags) -> Result<i32, CliError> {
    let audited = audit_project(target_dir)?;

    if flags.json {
        println!("{}", report_json::render_audit_result(&audited.result)?);
    } else {
        println!("{}", report_text::format_doctor_report(&audited.result));
    }

    Ok(exit_codes::audit_exit_code(&audited.result.summary))
}

fn run_fix_command(target_dir: &std::path::Path, flags: &Flags) -> Result<i32, CliError> {
    let initial = audit_project(target_dir)?;
    if initial.planned_fixes.len() != initial.result.fixes.len() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "one or more fixes are not executable from the Rust runtime yet",
        )));
    }

    let selector = FixSelector {
        ids: flags.fix_ids.clone(),
        prefixes: flags.fix_prefixes.clone(),
    };
    let planned = select_planned_fixes(&initial.planned_fixes, &selector);
    if !selector.is_empty() && planned.is_empty() {
        return Err(CliError::InvalidArguments(
            "No matching fixes for the requested selector.".to_string(),
        ));
    }
    let selected_fixes = select_fix_plans(&initial.result.fixes, &selector);
    let selected_initial = result_with_selected_fixes(&initial.result, selected_fixes.clone());
    let previewed = if flags.dry_run && flags.diff {
        Some(preview_fixes(&planned)?)
    } else {
        None
    };
    let applied = if flags.dry_run {
        Vec::new()
    } else {
        apply_fixes(&planned)?
    };
    let final_result = if flags.dry_run {
        selected_initial.clone()
    } else {
        audit_project(target_dir)?.result
    };

    if flags.json {
        println!(
            "{}",
            report_json::render_fix_result(
                flags.dry_run,
                target_dir,
                &selected_initial,
                &applied,
                &final_result,
                previewed.as_deref(),
            )?
        );
    } else {
        let selected_for_report = if selector.is_empty() && !flags.diff {
            None
        } else {
            Some(selected_initial.fixes.as_slice())
        };
        let preview_report = previewed
            .as_ref()
            .map(|previews| report_diff::render_fix_preview(target_dir, previews));
        println!(
            "{}",
            report_text::format_fix_result(
                flags.dry_run,
                target_dir,
                &selected_initial,
                &applied,
                &final_result,
                selected_for_report,
                preview_report.as_deref(),
            )
        );
    }

    Ok(exit_codes::fix_exit_code(&final_result.summary))
}

fn validate_command_flags(command: Option<&str>, flags: &Flags) -> Result<(), CliError> {
    let uses_fix_only_flags =
        flags.diff || !flags.fix_ids.is_empty() || !flags.fix_prefixes.is_empty();

    if uses_fix_only_flags && (flags.help || command != Some("fix")) {
        return Err(CliError::InvalidArguments(
            "Options \"--diff\", \"--fix-id\", and \"--fix-prefix\" are only available for \"fix\"."
                .to_string(),
        ));
    }

    if flags.diff && !flags.dry_run {
        return Err(CliError::InvalidArguments(
            "Option \"--diff\" requires \"fix --dry-run\".".to_string(),
        ));
    }

    Ok(())
}

fn resolve_target_dir(path_arg: Option<&OsStr>) -> io::Result<std::path::PathBuf> {
    match path_arg {
        Some(path) => std::path::absolute(std::path::PathBuf::from(path)),
        None => env::current_dir(),
    }
}

fn result_with_selected_fixes(result: &AuditResult, fixes: Vec<FixPlan>) -> AuditResult {
    let mut selected = result.clone();
    selected.summary.fixes_available = fixes.len();
    selected.fixes = fixes;
    selected
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::resolve_target_dir;

    #[test]
    fn resolve_target_dir_uses_absolute_current_dir_by_default() {
        let resolved = resolve_target_dir(None).expect("current dir should resolve");
        assert!(resolved.is_absolute());
    }

    #[test]
    fn resolve_target_dir_makes_relative_path_absolute() {
        let resolved = resolve_target_dir(Some(".".as_ref())).expect("path should resolve");
        assert!(resolved.is_absolute());
    }

    #[test]
    fn resolve_target_dir_keeps_absolute_paths() {
        let path = PathBuf::from("/tmp");
        let resolved = resolve_target_dir(Some(path.as_os_str())).expect("path should resolve");
        assert_eq!(resolved, path);
    }
}
