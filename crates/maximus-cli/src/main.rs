mod args;
mod exit_codes;
mod report_json;
mod report_text;

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io;
use std::process;

use maximus_checks::audit_project;
use maximus_core::apply_fixes;

use crate::args::{parse_args, Flags};

#[derive(Debug)]
enum CliError {
    Io(io::Error),
    Json(serde_json::Error),
    UnknownCommand(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::UnknownCommand(command) => {
                write!(f, "Unknown command \"{command}\". Run \"maximus help\" for usage.")
            }
        }
    }
}

impl Error for CliError {}

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
    let parsed = parse_args(argv);

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
    let planned = initial.planned_fixes.clone();
    let applied = if flags.dry_run {
        Vec::new()
    } else {
        apply_fixes(&planned)?
    };
    let final_result = if flags.dry_run {
        initial.result.clone()
    } else {
        audit_project(target_dir)?.result
    };

    if flags.json {
        println!(
            "{}",
            report_json::render_fix_result(
                flags.dry_run,
                target_dir,
                &initial.result,
                &applied,
                &final_result,
            )?
        );
    } else {
        println!(
            "{}",
            report_text::format_fix_result(
                flags.dry_run,
                target_dir,
                &initial.result,
                &applied,
                &final_result,
            )
        );
    }

    Ok(exit_codes::fix_exit_code(&final_result.summary))
}

fn resolve_target_dir(path_arg: Option<&OsStr>) -> io::Result<std::path::PathBuf> {
    match path_arg {
        Some(path) => std::path::absolute(std::path::PathBuf::from(path)),
        None => env::current_dir(),
    }
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
