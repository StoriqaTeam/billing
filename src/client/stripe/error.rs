use std::fmt;

use failure::{Backtrace, Context, Fail};
use serde_json;
use stripe::Error as StripeError;
use stripe::ParseIdError;
use validator::{ValidationError, ValidationErrors};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "stripe client error - malformed input")]
    MalformedInput,
    #[fail(display = "stripe client error - unauthorized")]
    Unauthorized,
    #[fail(display = "stripe client error - internal error")]
    Internal,
    #[fail(display = "stripe client error - bad request")]
    Validation(serde_json::Value),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "stripe client source - serde_json")]
    SerdeJson,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "stripe client context - currency is not fiat")]
    Currency,
}

derive_error_impls!();

impl From<StripeError> for Error {
    fn from(e: StripeError) -> Error {
        let kind: ErrorKind = e.into();
        kind.into()
    }
}

impl From<StripeError> for ErrorKind {
    fn from(err: StripeError) -> Self {
        match err {
            StripeError::Conversion(_)
            | StripeError::Http(_)
            | StripeError::Io(_)
            | StripeError::Unexpected(_)
            | StripeError::Unsupported(_) => ErrorKind::Internal,
            StripeError::Stripe(e) => {
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("stripe");
                if let Some(message) = e.message {
                    error.message = Some(message.into());
                }
                if let Some(code) = e.code {
                    error.add_param("code".into(), &code.to_string());
                }
                if let Some(charge) = e.charge {
                    error.add_param("charge".into(), &charge.to_string());
                }
                if let Some(decline_code) = e.decline_code {
                    error.add_param("decline_code".into(), &decline_code.to_string());
                }
                error.add_param("error_type".into(), &e.error_type.to_string());
                error.add_param("http_status".into(), &e.http_status.to_string());
                errors.add("request", error);
                ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())
            }
        }
    }
}

impl From<ParseIdError> for Error {
    fn from(e: ParseIdError) -> Error {
        let kind: ErrorKind = e.into();
        kind.into()
    }
}

impl From<ParseIdError> for ErrorKind {
    fn from(_err: ParseIdError) -> Self {
        ErrorKind::Internal
    }
}
