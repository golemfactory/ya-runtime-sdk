use crate::ErrorResponse;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct Error {
    code: i32,
    message: String,
    context: HashMap<String, String>,
}

impl Error {
    pub fn from_string(s: impl ToString) -> Self {
        Error {
            code: -1,
            message: s.to_string(),
            context: Default::default(),
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Self::from_string(e)
    }
}

impl From<ErrorResponse> for Error {
    fn from(e: ErrorResponse) -> Self {
        Error {
            code: e.code,
            message: e.message,
            context: e.context,
        }
    }
}

impl Into<ErrorResponse> for Error {
    fn into(self) -> ErrorResponse {
        ErrorResponse {
            code: self.code,
            message: self.message,
            context: self.context,
        }
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {})", self.message, self.code)
    }
}
