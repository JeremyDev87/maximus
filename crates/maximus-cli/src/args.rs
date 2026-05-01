use std::ffi::OsString;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flags {
    pub diff: bool,
    pub dry_run: bool,
    pub env_source_comments: bool,
    pub fail_on: Option<String>,
    pub fix_ids: Vec<String>,
    pub fix_prefixes: Vec<String>,
    pub help: bool,
    pub output_format: OutputFormat,
    pub output_path: Option<OsString>,
    pub only_checks: Option<Vec<String>>,
    pub skip_checks: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
    Sarif,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedArgs {
    pub command: Option<String>,
    pub args: Vec<OsString>,
    pub flags: Flags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    EmptyValue(&'static str),
    ConflictingValue(&'static str, &'static str),
    InvalidValue(&'static str, String, &'static str),
    MissingValue(&'static str),
}

impl Display for ArgsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyValue(flag) => write!(f, "\"{flag}\" 옵션에는 비어 있지 않은 값이 필요합니다."),
            Self::ConflictingValue(left, right) => write!(
                f,
                "\"{left}\" 옵션은 \"{right}\" 옵션과 함께 사용할 수 없습니다."
            ),
            Self::InvalidValue(flag, value, expected) => write!(
                f,
                "\"{flag}\" 옵션에 지원하지 않는 값 \"{value}\"가 전달되었습니다. 사용 가능한 값: {expected}."
            ),
            Self::MissingValue(flag) => write!(f, "\"{flag}\" 옵션에는 값이 필요합니다."),
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
    let mut output_format_source = None;
    let mut tokens = argv.into_iter().map(Into::into);

    while let Some(token) = tokens.next() {
        match token.to_str() {
            Some("--diff") => flags.diff = true,
            Some("--dry-run") => flags.dry_run = true,
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
            Some("--format") => {
                let value = next_option_value(tokens.next(), "--format")?;
                let output_format = parse_output_format(&value)?;
                set_output_format(
                    &mut flags,
                    &mut output_format_source,
                    output_format,
                    "--format",
                )?;
            }
            Some("--output") => {
                flags.output_path = Some(next_output_value(tokens.next())?);
            }
            Some("--skip") => {
                let value = next_option_value(tokens.next(), "--skip")?;
                let values = split_csv_values(&value, "--skip")?;
                flags
                    .skip_checks
                    .get_or_insert_with(Vec::new)
                    .extend(values);
            }
            Some("--json") => {
                set_output_format(
                    &mut flags,
                    &mut output_format_source,
                    OutputFormat::Json,
                    "--json",
                )?;
            }
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

fn next_output_value(value: Option<OsString>) -> Result<OsString, ArgsError> {
    let Some(value) = value else {
        return Err(ArgsError::MissingValue("--output"));
    };

    match value.to_str() {
        Some("") => Err(ArgsError::EmptyValue("--output")),
        Some(candidate) if candidate.starts_with('-') && candidate != "-" => {
            Err(ArgsError::MissingValue("--output"))
        }
        _ => Ok(value),
    }
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

fn parse_output_format(value: &OsString) -> Result<OutputFormat, ArgsError> {
    match value.to_string_lossy().as_ref() {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        "markdown" => Ok(OutputFormat::Markdown),
        "sarif" => Ok(OutputFormat::Sarif),
        value => Err(ArgsError::InvalidValue(
            "--format",
            value.to_string(),
            "text, json, markdown, sarif",
        )),
    }
}

fn set_output_format(
    flags: &mut Flags,
    source: &mut Option<&'static str>,
    output_format: OutputFormat,
    flag: &'static str,
) -> Result<(), ArgsError> {
    if source.is_some() && flags.output_format != output_format {
        return Err(ArgsError::ConflictingValue(
            source.unwrap_or("--format"),
            flag,
        ));
    }

    flags.output_format = output_format;
    *source = Some(flag);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_args, ArgsError, Flags, OutputFormat, ParsedArgs};

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
                    env_source_comments: false,
                    fail_on: None,
                    fix_ids: Vec::new(),
                    fix_prefixes: Vec::new(),
                    help: false,
                    output_format: OutputFormat::Json,
                    output_path: None,
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
            "--env-source-comments",
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
        assert!(parsed.flags.env_source_comments);
        assert_eq!(
            parsed.flags.fix_ids,
            vec!["env-example:create:.".to_string()]
        );
        assert_eq!(parsed.flags.fix_prefixes, vec!["env-example:".to_string()]);
        assert!(parsed.flags.diff);
    }

    #[test]
    fn parse_args_collects_output_path_and_stdout_marker() {
        let parsed = parse_args(["audit", "--json", "--output", "reports/audit.json"])
            .expect("args should parse");

        assert_eq!(
            parsed.flags.output_path,
            Some(OsString::from("reports/audit.json"))
        );

        let parsed = parse_args(["audit", "--format", "markdown", "--output", "-"])
            .expect("stdout marker should parse");

        assert_eq!(parsed.flags.output_path, Some(OsString::from("-")));
    }

    #[test]
    fn parse_args_errors_when_output_path_value_is_missing() {
        let error =
            parse_args(["audit", "--output", "--json"]).expect_err("output path should fail");

        assert_eq!(error, ArgsError::MissingValue("--output"));
    }

    #[test]
    fn parse_args_collects_format_output_values() {
        let parsed = parse_args(["audit", "--format", "markdown"]).expect("args should parse");
        assert_eq!(parsed.flags.output_format, OutputFormat::Markdown);

        let parsed = parse_args(["audit", "--format", "sarif"]).expect("args should parse");
        assert_eq!(parsed.flags.output_format, OutputFormat::Sarif);

        let parsed = parse_args(["audit", "--format", "json"]).expect("args should parse");
        assert_eq!(parsed.flags.output_format, OutputFormat::Json);

        let parsed = parse_args(["audit", "--format", "text"]).expect("args should parse");
        assert_eq!(parsed.flags.output_format, OutputFormat::Text);
    }

    #[test]
    fn parse_args_errors_when_output_format_flags_conflict() {
        let error = parse_args(["audit", "--json", "--format", "markdown"])
            .expect_err("conflicting output formats should fail");

        assert_eq!(error, ArgsError::ConflictingValue("--json", "--format"));
    }

    #[test]
    fn parse_args_errors_when_output_format_value_is_invalid() {
        let error = parse_args(["audit", "--format", "xml"])
            .expect_err("unknown output format should fail");

        assert_eq!(
            error,
            ArgsError::InvalidValue("--format", "xml".to_string(), "text, json, markdown, sarif")
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
