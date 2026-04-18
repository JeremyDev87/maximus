use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineRecord {
    pub id: String,
    pub file: PathBuf,
    pub detail_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineMatchKey<'a> {
    pub id: &'a str,
    pub file: &'a Path,
    pub detail_hash: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineError {
    Io {
        label: String,
        message: String,
    },
    Parse {
        label: String,
        message: String,
    },
}

impl BaselineError {
    fn io(label: impl Into<String>, error: std::io::Error) -> Self {
        Self::Io {
            label: label.into(),
            message: error.to_string(),
        }
    }

    fn parse(label: impl Into<String>, error: serde_json::Error) -> Self {
        Self::Parse {
            label: label.into(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for BaselineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaselineError::Io { label, message } | BaselineError::Parse { label, message } => {
                write!(formatter, "{}: {}", label, message)
            }
        }
    }
}

impl Error for BaselineError {}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BaselineDocument {
    Bare(Vec<BaselineRecord>),
    Wrapped { baseline: Vec<BaselineRecord> },
    Records { records: Vec<BaselineRecord> },
}

pub fn parse_baseline_records(
    text: &str,
    label: impl Into<String>,
) -> Result<Vec<BaselineRecord>, BaselineError> {
    let label = label.into();
    let document = serde_json::from_str::<BaselineDocument>(text)
        .map_err(|error| BaselineError::parse(label.clone(), error))?;

    Ok(match document {
        BaselineDocument::Bare(records) => records,
        BaselineDocument::Wrapped { baseline } => baseline,
        BaselineDocument::Records { records } => records,
    })
}

pub fn load_baseline_records(path: impl AsRef<Path>) -> Result<Vec<BaselineRecord>, BaselineError> {
    let path = path.as_ref();
    let label = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|error| BaselineError::io(label.clone(), error))?;

    parse_baseline_records(&text, label)
}

pub fn record_matches(record: &BaselineRecord, key: &BaselineMatchKey<'_>) -> bool {
    record.id == key.id
        && record.file == key.file
        && record.detail_hash == key.detail_hash
}

pub fn find_matching_record<'a>(
    records: &'a [BaselineRecord],
    key: &BaselineMatchKey<'_>,
) -> Option<&'a BaselineRecord> {
    records.iter().find(|record| record_matches(record, key))
}

pub fn contains_match(records: &[BaselineRecord], key: &BaselineMatchKey<'_>) -> bool {
    find_matching_record(records, key).is_some()
}

pub fn detail_hash(detail: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    for byte in detail.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }

    format!("{hash:016x}")
}
