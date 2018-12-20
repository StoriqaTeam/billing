use failure::{Backtrace, Context, Fail};
use serde_json;
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
    #[fail(display = "service error - forbidden")]
    Forbidden,
    #[fail(display = "service error - validation")]
    Validation(serde_json::Value),
}

derive_error_impls!();

impl From<RepoErrorKind> for ErrorKind {
    fn from(e: RepoErrorKind) -> Self {
        match e {
            RepoErrorKind::Constraints(errors) => ErrorKind::Validation(serde_json::to_value(errors).unwrap_or(json!({}))),
            RepoErrorKind::Forbidden => ErrorKind::Forbidden,
            RepoErrorKind::Internal => ErrorKind::Internal,
        }
    }
}

impl From<PaymentsClientErrorKind> for ErrorKind {
    fn from(e: PaymentsClientErrorKind) -> Self {
        match e {
            PaymentsClientErrorKind::Internal => ErrorKind::Internal,
            PaymentsClientErrorKind::MalformedInput => ErrorKind::Internal,
            PaymentsClientErrorKind::Unauthorized => ErrorKind::Internal,
            PaymentsClientErrorKind::Validation(value) => ErrorKind::Validation(value),
        }
    }
}
