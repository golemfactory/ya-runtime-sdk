use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde::Serialize;
use std::collections::HashMap;
use std::io;

use crate::ErrorResponse;

#[derive(Clone, Debug, Serialize)]
pub struct Error {
    code: i32,
    message: String,
    context: HashMap<String, String>,
}

impl Error {
    pub fn response<'a, T: 'a>(s: impl ToString) -> LocalBoxFuture<'a, Result<T, Self>> {
        let err = Self::from_string(s);
        futures::future::err(err).boxed_local()
    }

    pub fn from_string(s: impl ToString) -> Self {
        Error {
            code: 1,
            message: s.to_string(),
            context: Default::default(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error {
            code: e.raw_os_error().unwrap_or(1),
            message: e.to_string(),
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

impl From<Error> for ErrorResponse {
    fn from(e: Error) -> Self {
        ErrorResponse {
            code: e.code,
            message: e.message,
            context: e.context,
        }
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {})", self.message, self.code)
    }
}
