use std::error::Error;
use std::fmt;

use serde::de::DeserializeOwned;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseJsoncError {
    label: String,
    message: String,
}

impl ParseJsoncError {
    pub fn new(label: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            message: message.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseJsoncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.label, self.message)
    }
}

impl Error for ParseJsoncError {}

pub fn parse_jsonc<T>(text: &str, label: &str) -> Result<T, ParseJsoncError>
where
    T: DeserializeOwned,
{
    jsonc_parser::parse_to_serde_value(text, &Default::default())
        .map_err(|error| ParseJsoncError::new(label, error.to_string()))
}
