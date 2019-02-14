use failure::{Backtrace, Context, Fail};
use serde_json;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "payments client error - malformed input")]
    MalformedInput,
    #[fail(display = "payments client error - unauthorized")]
    Unauthorized,
    #[fail(display = "payments client error - internal error")]
    Internal,
    #[fail(display = "payments client error - unprocessable input")]
    Validation(serde_json::Value),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "payments client source - base64")]
    Base64,
    #[fail(display = "payments client source - jsonwebtoken")]
    JsonWebToken,
    #[fail(display = "payments client source - secp256k1")]
    Secp256k1,
    #[fail(display = "payments client source - serde_json")]
    SerdeJson,
    #[fail(display = "payments client source - stq_http")]
    StqHttp,
}

derive_error_impls!();
