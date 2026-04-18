mod args;
mod exit_codes;
mod report_json;
mod report_text;

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;

use serde::Serialize;

use crate::args::{parse_args, Flags};

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AppliedFix {
    pub id: String,
    pub title: String,
    pub files: Vec<PathBuf>,
    pub outcome: Option<String>,
}

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
        Some("audit") => delegate_to_js("audit", &target_dir, &parsed.flags),
        Some("doctor") => delegate_to_js("doctor", &target_dir, &parsed.flags),
        Some("fix") => delegate_to_js("fix", &target_dir, &parsed.flags),
        Some(command) => Err(CliError::UnknownCommand(command.to_string())),
        None => Ok(exit_codes::SUCCESS),
    }
}

fn resolve_target_dir(path_arg: Option<&OsStr>) -> io::Result<PathBuf> {
    match path_arg {
        Some(path) => std::path::absolute(PathBuf::from(path)),
        None => env::current_dir(),
    }
}

fn delegate_to_js(command: &str, target_dir: &Path, flags: &Flags) -> Result<i32, CliError> {
    let mut child = Command::new("node");
    child.arg(resolve_reference_cli_entrypoint()?);
    child.arg(command);
    child.arg(target_dir);

    if flags.dry_run {
        child.arg("--dry-run");
    }

    if flags.json {
        child.arg("--json");
    }

    let output = child.output()?;

    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(output.status.code().unwrap_or(exit_codes::FAILURE))
}

fn resolve_reference_cli_entrypoint() -> Result<PathBuf, CliError> {
    let search_roots = build_reference_cli_search_roots()?;

    find_reference_cli_from_roots(search_roots).ok_or_else(|| {
        CliError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "could not find bin/maximus.js from the current runtime context; run inside the repository checkout or set MAXIMUS_REPO_ROOT",
        ))
    })
}

fn build_reference_cli_search_roots() -> io::Result<Vec<PathBuf>> {
    let mut roots = Vec::new();

    if let Some(repo_root) = env::var_os("MAXIMUS_REPO_ROOT") {
        roots.push(PathBuf::from(repo_root));
    }

    roots.extend(ancestor_paths(env::current_exe()?));

    Ok(roots)
}

fn ancestor_paths(path: PathBuf) -> Vec<PathBuf> {
    let start = if path.is_file() {
        path.parent().map(Path::to_path_buf).unwrap_or(path)
    } else {
        path
    };

    start.ancestors().map(Path::to_path_buf).collect()
}

fn find_reference_cli_from_roots<I>(roots: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    for root in roots {
        let candidate = root.join("bin/maximus.js");
        if candidate.is_file() && root.join("package.json").is_file() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{ancestor_paths, find_reference_cli_from_roots};

    #[test]
    fn ancestor_paths_include_parent_chain_for_files() {
        let path = PathBuf::from("/tmp/repo/target/debug/maximus");
        let ancestors = ancestor_paths(path);

        assert!(ancestors.contains(&PathBuf::from("/tmp/repo/target/debug")));
        assert!(ancestors.contains(&PathBuf::from("/tmp/repo/target")));
        assert!(ancestors.contains(&PathBuf::from("/tmp/repo")));
    }

    #[test]
    fn reference_cli_path_is_found_from_nested_runtime_roots() {
        let temp = tempdir().expect("temp dir should exist");
        let repo_root = temp.path().join("repo");
        let nested_dir = repo_root.join("target/debug/deps");
        let bin_dir = repo_root.join("bin");

        fs::create_dir_all(&nested_dir).expect("nested dir should exist");
        fs::create_dir_all(&bin_dir).expect("bin dir should exist");
        fs::write(repo_root.join("package.json"), "{}").expect("package.json should exist");
        fs::write(bin_dir.join("maximus.js"), "console.log('ok');")
            .expect("reference cli should exist");

        let found = find_reference_cli_from_roots(ancestor_paths(nested_dir))
            .expect("reference cli should be found");

        assert_eq!(found, repo_root.join("bin/maximus.js"));
    }

    #[test]
    fn reference_cli_path_is_absent_when_repo_markers_are_missing() {
        let temp = tempdir().expect("temp dir should exist");
        let root = temp.path().join("random");

        fs::create_dir_all(root.join("bin")).expect("bin dir should exist");
        fs::write(root.join("bin/maximus.js"), "console.log('ok');")
            .expect("script should exist");

        assert!(find_reference_cli_from_roots([root]).is_none());
    }

    #[test]
    fn search_roots_prefer_current_exe_ancestors_over_current_dir() {
        let temp = tempdir().expect("temp dir should exist");
        let right_repo = temp.path().join("right-repo");
        let wrong_repo = temp.path().join("wrong-repo");
        let exe_dir = right_repo.join("target/debug");

        fs::create_dir_all(right_repo.join("bin")).expect("right bin dir should exist");
        fs::create_dir_all(wrong_repo.join("bin")).expect("wrong bin dir should exist");
        fs::create_dir_all(&exe_dir).expect("exe dir should exist");
        fs::write(right_repo.join("package.json"), "{}").expect("right package.json should exist");
        fs::write(wrong_repo.join("package.json"), "{}").expect("wrong package.json should exist");
        fs::write(right_repo.join("bin/maximus.js"), "console.log('right');")
            .expect("right maximus should exist");
        fs::write(wrong_repo.join("bin/maximus.js"), "console.log('wrong');")
            .expect("wrong maximus should exist");

        let roots = ancestor_paths(exe_dir)
            .into_iter()
            .chain(ancestor_paths(wrong_repo.clone()))
            .collect::<Vec<_>>();

        let found = find_reference_cli_from_roots(roots).expect("reference cli should be found");

        assert_eq!(found, right_repo.join("bin/maximus.js"));
    }
}
