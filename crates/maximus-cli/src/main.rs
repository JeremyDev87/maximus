mod args;
mod exit_codes;
mod fail_policy;
mod report_diff;
mod report_json;
mod report_text;

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io;
use std::process;

use maximus_checks::{audit_project_with_config_root, registered_check_ids};
use maximus_core::{
    apply_fixes, load_maximus_config, preview_fixes, select_fix_plans, select_planned_fixes,
    AuditResult, FailOnLevel, FixPlan, FixSelector, LoadConfigError, MaximusConfig,
};

use crate::args::{parse_args, ArgsError, Flags};

#[derive(Debug, Clone)]
struct ResolvedConfig {
    config: MaximusConfig,
    ignore_root: std::path::PathBuf,
}

#[derive(Debug)]
enum CliError {
    Args(ArgsError),
    Config(LoadConfigError),
    Io(io::Error),
    Json(serde_json::Error),
    InvalidArguments(String),
    UnknownCommand(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Args(error) => write!(f, "{error}"),
            Self::Config(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::InvalidArguments(message) => write!(f, "{message}"),
            Self::UnknownCommand(command) => {
                write!(
                    f,
                    "Unknown command \"{command}\". Run \"maximus help\" for usage."
                )
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

impl From<LoadConfigError> for CliError {
    fn from(value: LoadConfigError) -> Self {
        Self::Config(value)
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
    let show_default_help = parsed.command.is_none() && parsed.flags == Flags::default();

    if show_default_help || parsed.command.as_deref() == Some("help") || parsed.flags.help {
        println!("{}", report_text::format_help());
        return Ok(exit_codes::SUCCESS);
    }

    validate_command_flags(parsed.command.as_deref(), &parsed.flags)?;

    if !matches!(
        parsed.command.as_deref(),
        Some("audit") | Some("doctor") | Some("fix")
    ) {
        return Err(CliError::UnknownCommand(
            parsed.command.as_deref().unwrap_or_default().to_string(),
        ));
    }

    let target_dir = resolve_target_dir(parsed.args.first().map(|value| value.as_os_str()))?;
    let config = resolve_effective_config(&target_dir, &parsed.flags)?;

    match parsed.command.as_deref() {
        Some("audit") => run_audit_command(&target_dir, &parsed.flags, &config),
        Some("doctor") => run_doctor_command(&target_dir, &parsed.flags, &config),
        Some("fix") => run_fix_command(&target_dir, &parsed.flags, &config),
        Some(command) => Err(CliError::UnknownCommand(command.to_string())),
        None => Ok(exit_codes::SUCCESS),
    }
}

fn run_audit_command(
    target_dir: &std::path::Path,
    flags: &Flags,
    resolved: &ResolvedConfig,
) -> Result<i32, CliError> {
    let audited =
        audit_project_with_config_root(target_dir, &resolved.config, &resolved.ignore_root)?;

    if flags.json {
        println!("{}", report_json::render_audit_result(&audited.result)?);
    } else {
        println!("{}", report_text::format_audit_report(&audited.result));
    }

    Ok(fail_policy::exit_code(
        &audited.result.summary,
        resolved
            .config
            .report
            .fail_on
            .as_ref()
            .unwrap_or(&FailOnLevel::Warn),
    ))
}

fn run_doctor_command(
    target_dir: &std::path::Path,
    flags: &Flags,
    resolved: &ResolvedConfig,
) -> Result<i32, CliError> {
    let audited =
        audit_project_with_config_root(target_dir, &resolved.config, &resolved.ignore_root)?;

    if flags.json {
        println!("{}", report_json::render_audit_result(&audited.result)?);
    } else {
        println!("{}", report_text::format_doctor_report(&audited.result));
    }

    Ok(fail_policy::exit_code(
        &audited.result.summary,
        resolved
            .config
            .report
            .fail_on
            .as_ref()
            .unwrap_or(&FailOnLevel::Warn),
    ))
}

fn run_fix_command(
    target_dir: &std::path::Path,
    flags: &Flags,
    resolved: &ResolvedConfig,
) -> Result<i32, CliError> {
    let initial =
        audit_project_with_config_root(target_dir, &resolved.config, &resolved.ignore_root)?;
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
        audit_project_with_config_root(target_dir, &resolved.config, &resolved.ignore_root)?.result
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

    Ok(fail_policy::exit_code(
        &final_result.summary,
        resolved
            .config
            .report
            .fail_on
            .as_ref()
            .unwrap_or(&FailOnLevel::Warn),
    ))
}

fn resolve_effective_config(
    target_dir: &std::path::Path,
    flags: &Flags,
) -> Result<ResolvedConfig, CliError> {
    let loaded = load_maximus_config(target_dir)?;
    let ignore_root = loaded
        .as_ref()
        .and_then(|loaded| loaded.path.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| target_dir.to_path_buf());
    let mut config = loaded.map(|loaded| loaded.config).unwrap_or_default();

    validate_check_ids("only", &config.checks.only)?;
    validate_check_ids("skip", &config.checks.skip)?;

    if flags.only_checks.is_some() || flags.skip_checks.is_some() {
        config.checks = maximus_core::CheckFilterConfig::default();
    }

    if let Some(only_checks) = &flags.only_checks {
        config.checks.only = only_checks.clone();
    }
    if let Some(skip_checks) = &flags.skip_checks {
        config.checks.skip = skip_checks.clone();
    }
    if let Some(fail_on) = &flags.fail_on {
        config.report.fail_on = Some(parse_fail_on_level(fail_on)?);
    }

    validate_check_ids("only", &config.checks.only)?;
    validate_check_ids("skip", &config.checks.skip)?;

    Ok(ResolvedConfig {
        config,
        ignore_root,
    })
}

fn parse_fail_on_level(value: &str) -> Result<FailOnLevel, CliError> {
    match value {
        "error" => Ok(FailOnLevel::Error),
        "warn" => Ok(FailOnLevel::Warn),
        "info" => Ok(FailOnLevel::Info),
        "none" => Ok(FailOnLevel::None),
        _ => Err(CliError::InvalidArguments(format!(
            "Unknown fail-on level \"{value}\". Use one of: error, warn, info, none."
        ))),
    }
}

fn validate_check_ids(source: &str, ids: &[String]) -> Result<(), CliError> {
    let known_ids = registered_check_ids();

    for id in ids {
        if !known_ids.contains(&id.as_str()) {
            return Err(CliError::InvalidArguments(format!(
                "Unknown check id \"{id}\" in {source}. Use one of: {}.",
                known_ids.join(", ")
            )));
        }
    }

    Ok(())
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
