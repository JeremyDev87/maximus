use std::ffi::OsString;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flags {
    pub diff: bool,
    pub dry_run: bool,
    pub fix_ids: Vec<String>,
    pub fix_prefixes: Vec<String>,
    pub help: bool,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedArgs {
    pub command: Option<String>,
    pub args: Vec<OsString>,
    pub flags: Flags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    MissingValue(&'static str),
}

impl Display for ArgsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "Option \"{flag}\" requires a value."),
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
            Some("--fix-id") => {
                let value = next_option_value(tokens.next(), "--fix-id")?;
                flags.fix_ids.push(value.to_string_lossy().into_owned());
            }
            Some("--fix-prefix") => {
                let value = next_option_value(tokens.next(), "--fix-prefix")?;
                flags.fix_prefixes.push(value.to_string_lossy().into_owned());
            }
            Some("--json") => flags.json = true,
            Some("--help") | Some("-h") => flags.help = true,
            _ => args.push(token),
        }
    }

    Ok(ParsedArgs {
        command: args.first().map(|value| value.to_string_lossy().into_owned()),
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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_args, ArgsError, Flags, ParsedArgs};

    #[test]
    fn parse_args_collects_known_flags_and_positionals() {
        let parsed = parse_args(["fix", "./repo", "--dry-run", "--json"])
            .expect("args should parse");

        assert_eq!(
            parsed,
            ParsedArgs {
                command: Some("fix".to_string()),
                args: vec![OsString::from("./repo")],
                flags: Flags {
                    diff: false,
                    dry_run: true,
                    fix_ids: Vec::new(),
                    fix_prefixes: Vec::new(),
                    help: false,
                    json: true,
                },
            }
        );
    }

    #[test]
    fn parse_args_treats_unknown_tokens_as_positionals() {
        let parsed =
            parse_args(["audit", "--mystery", "target"]).expect("args should parse");

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
            "--fix-id",
            "env-example:create:.",
            "--fix-prefix",
            "env-example:",
            "--diff",
        ])
        .expect("args should parse");

        assert_eq!(parsed.command.as_deref(), Some("fix"));
        assert_eq!(parsed.args, vec![OsString::from("./repo")]);
        assert_eq!(parsed.flags.fix_ids, vec!["env-example:create:.".to_string()]);
        assert_eq!(parsed.flags.fix_prefixes, vec!["env-example:".to_string()]);
        assert!(parsed.flags.diff);
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
