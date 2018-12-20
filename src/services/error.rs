use failure::{Backtrace, Context, Fail};
use std::fmt;

use client::payments::ErrorKind as PaymentsClientErrorKind;
use repos::ErrorKind as RepoErrorKind;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "service error - internal")]
    Internal,
}

derive_error_impls!();

impl From<RepoErrorKind> for ErrorKind {
    fn from(_e: RepoErrorKind) -> Self {
        // TODO: map error correctly
        ErrorKind::Internal
    }
}

impl From<PaymentsClientErrorKind> for ErrorKind {
    fn from(_e: PaymentsClientErrorKind) -> Self {
        // TODO: map error correctly
        ErrorKind::Internal
    }
}
