use std::fmt;

use diesel::result::{DatabaseErrorKind, Error as DieselError};
use failure::{Backtrace, Context, Fail};
use validator::{ValidationError, ValidationErrors};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "repo error - violation of constraints: {}", _0)]
    Constraints(ValidationErrors),
    #[fail(display = "repo error - internal")]
    Internal,
    #[fail(display = "repo error - access forbidden")]
    Forbidden,
    #[fail(display = "repo error - not found")]
    NotFound,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "repo source - Diesel")]
    Diesel,
    #[fail(display = "repo source - R2D2")]
    R2d2,
    #[fail(display = "repo source - serde_json")]
    SerdeJson,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "repo context - error getting database connection")]
    Connection,
}

derive_error_impls!();

impl<'a> From<&'a DieselError> for ErrorKind {
    fn from(e: &DieselError) -> Self {
        match e {
            DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, ref info) => {
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("not unique");
                let message: &str = info.message();
                error.add_param("message".into(), &message);
                errors.add("repo", error);
                ErrorKind::Constraints(errors)
            }
            DieselError::NotFound => ErrorKind::NotFound,
            _ => ErrorKind::Internal,
        }
    }
}

impl From<DieselError> for Error {
    fn from(e: DieselError) -> Self {
        ectx!(err ErrorSource::Diesel, ErrorKind::from(&e))
    }
}
