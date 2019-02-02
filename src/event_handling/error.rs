use diesel::result::Error as DieselError;
use failure::{Backtrace, Context, Fail};
use std::fmt;

use client::stores::ErrorKind as StoresErrorKind;
use client::stripe::ErrorKind as StripeErrorKind;
use repos::error::ErrorKind as RepoErrorKind;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Debug, Clone, Fail, PartialEq)]
pub enum ErrorKind {
    #[fail(display = "event handler error - internal")]
    Internal,
    #[fail(display = "event handler error - error currency conversion")]
    CurrencyConversion,
    #[fail(display = "event handler error - this event has already been completed")]
    AlreadyDone,
}

#[derive(Debug, Clone, Fail, PartialEq, Eq)]
pub enum ErrorSource {
    #[fail(display = "event handler source - serde_json")]
    SerdeJson,
    #[fail(display = "event handler source - tokio_timer")]
    TokioTimer,
    #[fail(display = "event handler source - r2d2")]
    R2d2,
}

derive_error_impls!();

impl<'a> From<&'a DieselError> for ErrorKind {
    fn from(_e: &DieselError) -> Self {
        ErrorKind::Internal
    }
}

impl From<RepoErrorKind> for ErrorKind {
    fn from(_e: RepoErrorKind) -> Self {
        ErrorKind::Internal
    }
}

impl From<StoresErrorKind> for ErrorKind {
    fn from(_e: StoresErrorKind) -> Self {
        ErrorKind::Internal
    }
}

impl From<StripeErrorKind> for ErrorKind {
    fn from(_e: StripeErrorKind) -> Self {
        ErrorKind::Internal
    }
}
