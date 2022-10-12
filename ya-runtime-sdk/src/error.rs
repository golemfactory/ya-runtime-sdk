use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde::Serialize;
use std::collections::HashMap;
use std::io;

use crate::ErrorResponse;

pub trait ErrorExt<T> {
    fn or_err(self, s: impl ToString) -> Result<T, Error>;
}

impl<T> ErrorExt<T> for Option<T> {
    fn or_err(self, s: impl ToString) -> Result<T, Error> {
        match self {
            Some(t) => Ok(t),
            None => Err(Error::from(s.to_string())),
        }
    }
}

impl<T, E> ErrorExt<T> for Result<T, E>
where
    E: Into<Error> + std::fmt::Display,
{
    fn or_err(self, s: impl ToString) -> Result<T, Error> {
        match self {
            Ok(t) => Ok(t),
            Err(e) => Err(Error::from(format!("{}: {}", s.to_string(), e))),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Error {
    code: i32,
    message: String,
    context: HashMap<String, String>,
}

impl Error {
    pub fn response<'a, T: 'a>(s: impl ToString) -> LocalBoxFuture<'a, Result<T, Self>> {
        let err = Self::from(s.to_string());
        futures::future::err(err).boxed_local()
    }

    pub fn from_string(s: impl ToString) -> Self {
        Self::from(s.to_string())
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error {
            code: 1,
            message,
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
