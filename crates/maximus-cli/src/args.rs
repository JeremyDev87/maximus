use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flags {
    pub dry_run: bool,
    pub help: bool,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedArgs {
    pub command: Option<String>,
    pub args: Vec<OsString>,
    pub flags: Flags,
}

pub fn parse_args<I, S>(argv: I) -> ParsedArgs
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = Vec::new();
    let mut flags = Flags::default();

    for token in argv.into_iter().map(Into::into) {
        match token.to_str() {
            Some("--dry-run") => flags.dry_run = true,
            Some("--json") => flags.json = true,
            Some("--help") | Some("-h") => flags.help = true,
            _ => args.push(token),
        }
    }

    ParsedArgs {
        command: args.first().map(|value| value.to_string_lossy().into_owned()),
        args: args.into_iter().skip(1).collect(),
        flags,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_args, Flags, ParsedArgs};

    #[test]
    fn parse_args_collects_known_flags_and_positionals() {
        let parsed = parse_args(["fix", "./repo", "--dry-run", "--json"]);

        assert_eq!(
            parsed,
            ParsedArgs {
                command: Some("fix".to_string()),
                args: vec![OsString::from("./repo")],
                flags: Flags {
                    dry_run: true,
                    help: false,
                    json: true,
                },
            }
        );
    }

    #[test]
    fn parse_args_treats_unknown_tokens_as_positionals() {
        let parsed = parse_args(["audit", "--mystery", "target"]);

        assert_eq!(parsed.command.as_deref(), Some("audit"));
        assert_eq!(
            parsed.args,
            vec![OsString::from("--mystery"), OsString::from("target")]
        );
    }

    #[cfg(unix)]
    #[test]
    fn parse_args_keeps_non_utf8_positionals_without_panicking() {
        use std::os::unix::ffi::OsStringExt;

        let parsed = parse_args([
            OsString::from("audit"),
            OsString::from_vec(vec![0x66, 0x6f, 0x80, 0x6f]),
        ]);

        assert_eq!(parsed.command.as_deref(), Some("audit"));
        assert_eq!(parsed.args.len(), 1);
    }
}
