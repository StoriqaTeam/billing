use std::fmt;

use failure::{Backtrace, Context, Fail};
use serde_json;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "stores client error - malformed input")]
    MalformedInput,
    #[fail(display = "stores client error - unauthorized")]
    Unauthorized,
    #[fail(display = "stores client error - internal error")]
    Internal,
    #[fail(display = "stores client error - bad request")]
    Validation(serde_json::Value),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "stores client source - serde_json")]
    SerdeJson,
    #[fail(display = "stores client source - stq_http")]
    StqHttp,
}

derive_error_impls!();
