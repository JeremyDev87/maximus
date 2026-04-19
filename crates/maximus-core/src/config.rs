use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{parse_jsonc, read_text_if_exists, ParseJsoncError};

const CONFIG_FILE_NAMES: [&str; 2] = ["maximus.config.json", ".maximusrc.json"];

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaximusConfig {
    #[serde(default)]
    pub checks: CheckFilterConfig,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub severity: BTreeMap<String, ConfigSeverity>,
    #[serde(default)]
    pub report: ReportConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckFilterConfig {
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSeverity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailOnLevel {
    Error,
    Warn,
    Info,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportConfig {
    #[serde(default, rename = "failOn")]
    pub fail_on: Option<FailOnLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: MaximusConfig,
}

#[derive(Debug)]
pub enum LoadConfigError {
    Io(io::Error),
    Parse(ParseJsoncError),
}

impl std::fmt::Display for LoadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LoadConfigError {}

impl From<io::Error> for LoadConfigError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ParseJsoncError> for LoadConfigError {
    fn from(value: ParseJsoncError) -> Self {
        Self::Parse(value)
    }
}

pub fn load_maximus_config(start_dir: &Path) -> Result<Option<LoadedConfig>, LoadConfigError> {
    let Some(config_path) = find_maximus_config_path(start_dir)? else {
        return Ok(None);
    };
    let Some(text) = read_text_if_exists(&config_path)? else {
        return Ok(None);
    };
    let config = parse_jsonc::<MaximusConfig>(&text, &config_path.to_string_lossy())?;

    Ok(Some(LoadedConfig {
        path: config_path,
        config,
    }))
}

pub fn find_maximus_config_path(start_dir: &Path) -> io::Result<Option<PathBuf>> {
    let start_dir = std::path::absolute(start_dir)?;

    for directory in start_dir.ancestors() {
        for file_name in CONFIG_FILE_NAMES {
            let candidate = directory.join(file_name);
            if candidate.is_file() {
                return Ok(Some(candidate));
            }
        }
    }

    Ok(None)
}
