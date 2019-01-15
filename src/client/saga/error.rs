use std::fmt;

use failure::{Backtrace, Context, Fail};
use serde_json;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "saga client error - malformed input")]
    MalformedInput,
    #[fail(display = "saga client error - unauthorized")]
    Unauthorized,
    #[fail(display = "saga client error - internal error")]
    Internal,
    #[fail(display = "saga client error - bad request")]
    Validation(serde_json::Value),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "saga client source - serde_json")]
    SerdeJson,
    #[fail(display = "saga client source - stq_http")]
    StqHttp,
}

derive_error_impls!();
