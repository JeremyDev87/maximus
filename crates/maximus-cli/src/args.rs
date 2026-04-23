use std::ffi::OsString;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flags {
    pub diff: bool,
    pub dry_run: bool,
    pub env_group_sort: Option<EnvGroupSortMode>,
    pub env_source_comments: bool,
    pub fail_on: Option<String>,
    pub fix_ids: Vec<String>,
    pub fix_prefixes: Vec<String>,
    pub help: bool,
    pub json: bool,
    pub only_checks: Option<Vec<String>>,
    pub skip_checks: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedArgs {
    pub command: Option<String>,
    pub args: Vec<OsString>,
    pub flags: Flags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvGroupSortMode {
    Plain,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    EmptyValue(&'static str),
    MissingValue(&'static str),
    InvalidValue(&'static str, String),
}

impl Display for ArgsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyValue(flag) => write!(f, "Option \"{flag}\" requires a non-empty value."),
            Self::MissingValue(flag) => write!(f, "Option \"{flag}\" requires a value."),
            Self::InvalidValue(flag, value) => {
                write!(f, "Option \"{flag}\" does not accept value \"{value}\".")
            }
        }
    }
}

pub fn parse_args<I, S>(argv: I) -> Result<ParsedArgs, ArgsError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = Vec::new();
    let mut flags = Flags::default();
    let mut tokens = argv.into_iter().map(Into::into);

    while let Some(token) = tokens.next() {
        match token.to_str() {
            Some("--diff") => flags.diff = true,
            Some("--dry-run") => flags.dry_run = true,
            Some("--env-group-sort") => {
                let value = next_option_value(tokens.next(), "--env-group-sort")?;
                flags.env_group_sort = Some(parse_env_group_sort(&value)?);
            }
            Some("--env-source-comments") => flags.env_source_comments = true,
            Some("--fail-on") => {
                let value = next_option_value(tokens.next(), "--fail-on")?;
                flags.fail_on = Some(value.to_string_lossy().into_owned());
            }
            Some("--fix-id") => {
                let value = next_option_value(tokens.next(), "--fix-id")?;
                flags.fix_ids.push(value.to_string_lossy().into_owned());
            }
            Some("--fix-prefix") => {
                let value = next_option_value(tokens.next(), "--fix-prefix")?;
                flags
                    .fix_prefixes
                    .push(value.to_string_lossy().into_owned());
            }
            Some("--only") => {
                let value = next_option_value(tokens.next(), "--only")?;
                let values = split_csv_values(&value, "--only")?;
                flags
                    .only_checks
                    .get_or_insert_with(Vec::new)
                    .extend(values);
            }
            Some("--skip") => {
                let value = next_option_value(tokens.next(), "--skip")?;
                let values = split_csv_values(&value, "--skip")?;
                flags
                    .skip_checks
                    .get_or_insert_with(Vec::new)
                    .extend(values);
            }
            Some("--json") => flags.json = true,
            Some("--help") | Some("-h") => flags.help = true,
            _ => args.push(token),
        }
    }

    Ok(ParsedArgs {
        command: args
            .first()
            .map(|value| value.to_string_lossy().into_owned()),
        args: args.into_iter().skip(1).collect(),
        flags,
    })
}

fn next_option_value(value: Option<OsString>, flag: &'static str) -> Result<OsString, ArgsError> {
    let Some(value) = value else {
        return Err(ArgsError::MissingValue(flag));
    };

    if value
        .to_str()
        .is_some_and(|candidate| candidate.starts_with('-'))
    {
        return Err(ArgsError::MissingValue(flag));
    }

    Ok(value)
}

fn split_csv_values(value: &OsString, flag: &'static str) -> Result<Vec<String>, ArgsError> {
    let values = value
        .to_string_lossy()
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if values.is_empty() {
        return Err(ArgsError::EmptyValue(flag));
    }

    Ok(values)
}

fn parse_env_group_sort(value: &OsString) -> Result<EnvGroupSortMode, ArgsError> {
    match value.to_string_lossy().as_ref() {
        "none" => Ok(EnvGroupSortMode::Plain),
        "prefix" => Ok(EnvGroupSortMode::Prefix),
        other => Err(ArgsError::InvalidValue(
            "--env-group-sort",
            other.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_args, ArgsError, EnvGroupSortMode, Flags, ParsedArgs};

    #[test]
    fn parse_args_collects_known_flags_and_positionals() {
        let parsed =
            parse_args(["fix", "./repo", "--dry-run", "--json"]).expect("args should parse");

        assert_eq!(
            parsed,
            ParsedArgs {
                command: Some("fix".to_string()),
                args: vec![OsString::from("./repo")],
                flags: Flags {
                    diff: false,
                    dry_run: true,
                    env_group_sort: None,
                    env_source_comments: false,
                    fail_on: None,
                    fix_ids: Vec::new(),
                    fix_prefixes: Vec::new(),
                    help: false,
                    json: true,
                    only_checks: None,
                    skip_checks: None,
                },
            }
        );
    }

    #[test]
    fn parse_args_treats_unknown_tokens_as_positionals() {
        let parsed = parse_args(["audit", "--mystery", "target"]).expect("args should parse");

        assert_eq!(parsed.command.as_deref(), Some("audit"));
        assert_eq!(
            parsed.args,
            vec![OsString::from("--mystery"), OsString::from("target")]
        );
    }

    #[test]
    fn parse_args_collects_fix_selectors_and_diff_flag() {
        let parsed = parse_args([
            "fix",
            "./repo",
            "--only",
            "env,tsconfig",
            "--skip",
            "duplicates",
            "--fail-on",
            "error",
            "--fix-id",
            "env-example:create:.",
            "--fix-prefix",
            "env-example:",
            "--diff",
        ])
        .expect("args should parse");

        assert_eq!(parsed.command.as_deref(), Some("fix"));
        assert_eq!(parsed.args, vec![OsString::from("./repo")]);
        assert_eq!(
            parsed.flags.only_checks,
            Some(vec!["env".to_string(), "tsconfig".to_string()])
        );
        assert_eq!(
            parsed.flags.skip_checks,
            Some(vec!["duplicates".to_string()])
        );
        assert_eq!(parsed.flags.fail_on.as_deref(), Some("error"));
        assert_eq!(
            parsed.flags.fix_ids,
            vec!["env-example:create:.".to_string()]
        );
        assert_eq!(parsed.flags.fix_prefixes, vec!["env-example:".to_string()]);
        assert!(parsed.flags.diff);
    }

    #[test]
    fn parse_args_collects_env_template_render_flags() {
        let parsed = parse_args([
            "fix",
            "./repo",
            "--env-group-sort",
            "prefix",
            "--env-source-comments",
        ])
        .expect("args should parse");

        assert_eq!(parsed.flags.env_group_sort, Some(EnvGroupSortMode::Prefix));
        assert!(parsed.flags.env_source_comments);
    }

    #[test]
    fn parse_args_errors_when_env_group_sort_value_is_invalid() {
        let error = parse_args(["fix", "--env-group-sort", "alphabetical"])
            .expect_err("invalid env group sort should fail");

        assert_eq!(
            error,
            ArgsError::InvalidValue("--env-group-sort", "alphabetical".to_string())
        );
    }

    #[test]
    fn parse_args_errors_when_fix_selector_value_is_missing() {
        let error = parse_args(["fix", "--fix-id"]).expect_err("missing selector should fail");

        assert_eq!(error, ArgsError::MissingValue("--fix-id"));
    }

    #[test]
    fn parse_args_errors_when_fix_selector_value_is_another_flag() {
        let error = parse_args(["fix", "--fix-id", "--dry-run"])
            .expect_err("flag-shaped selector should fail");

        assert_eq!(error, ArgsError::MissingValue("--fix-id"));
    }

    #[test]
    fn parse_args_errors_when_only_filter_is_empty() {
        let error =
            parse_args(["audit", "--only", "  ,  "]).expect_err("empty only filter should fail");

        assert_eq!(error, ArgsError::EmptyValue("--only"));
    }

    #[test]
    fn parse_args_errors_when_skip_filter_is_empty() {
        let error = parse_args(["audit", "--skip", ""]).expect_err("empty skip filter should fail");

        assert_eq!(error, ArgsError::EmptyValue("--skip"));
    }

    #[test]
    fn parse_args_collects_multiple_check_filter_flags() {
        let parsed = parse_args([
            "audit",
            ".",
            "--only",
            "env",
            "--only",
            "tsconfig,duplicates",
            "--skip",
            "lockfiles",
        ])
        .expect("args should parse");

        assert_eq!(
            parsed.flags.only_checks,
            Some(vec![
                "env".to_string(),
                "tsconfig".to_string(),
                "duplicates".to_string()
            ])
        );
        assert_eq!(
            parsed.flags.skip_checks,
            Some(vec!["lockfiles".to_string()])
        );
    }

    #[cfg(unix)]
    #[test]
    fn parse_args_keeps_non_utf8_positionals_without_panicking() {
        use std::os::unix::ffi::OsStringExt;

        let parsed = parse_args([
            OsString::from("audit"),
            OsString::from_vec(vec![0x66, 0x6f, 0x80, 0x6f]),
        ])
        .expect("args should parse");

        assert_eq!(parsed.command.as_deref(), Some("audit"));
        assert_eq!(parsed.args.len(), 1);
    }
}
