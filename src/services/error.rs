use diesel::result::Error as DieselError;
use failure::{Backtrace, Context, Fail};
use serde_json;
use std::fmt;
use stripe::WebhookError;

use client::payments::ErrorKind as PaymentsClientErrorKind;
use client::stripe::ErrorKind as StripeClientErrorKind;
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

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "service context - error amount conversion")]
    AmountConversion,
    #[fail(display = "service context - wrong order state")]
    OrderState,
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

impl From<DieselError> for Error {
    fn from(e: DieselError) -> Self {
        Error {
            inner: ErrorKind::from(&e).into(),
        }
    }
}

impl<'a> From<&'a DieselError> for ErrorKind {
    fn from(_e: &DieselError) -> Self {
        ErrorKind::Internal
    }
}

impl From<WebhookError> for Error {
    fn from(e: WebhookError) -> Error {
        Error {
            inner: ErrorKind::from(&e).into(),
        }
    }
}

impl<'a> From<&'a WebhookError> for ErrorKind {
    fn from(_e: &WebhookError) -> Self {
        ErrorKind::Internal
    }
}

impl From<StripeClientErrorKind> for ErrorKind {
    fn from(e: StripeClientErrorKind) -> Self {
        match e {
            StripeClientErrorKind::Internal => ErrorKind::Internal,
            StripeClientErrorKind::MalformedInput => ErrorKind::Internal,
            StripeClientErrorKind::Unauthorized => ErrorKind::Internal,
            StripeClientErrorKind::Validation(value) => ErrorKind::Validation(value),
        }
    }
}
