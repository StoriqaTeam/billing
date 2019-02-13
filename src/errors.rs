use hyper::StatusCode;
use serde_json;
use validator::ValidationErrors;

use stq_http::errors::{Codeable, PayloadCarrier};

use services;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Not found")]
    NotFound,
    #[fail(display = "Parse error")]
    Parse,
    #[fail(display = "Validation error")]
    Validate(ValidationErrors),
    #[fail(display = "Server is refusing to fullfil the request")]
    Forbidden,
    #[fail(display = "R2D2 connection error")]
    Connection,
    #[fail(display = "Http Client error")]
    HttpClient,
    #[fail(display = "Invalid oauth token")]
    InvalidToken,
    #[fail(display = "Internal error (error handling v2)")]
    InternalV2,
    #[fail(display = "Validation error (error handling v2)")]
    ValidateV2(serde_json::Value),
}

impl From<services::Error> for Error {
    fn from(error: services::Error) -> Error {
        match error.kind() {
            services::ErrorKind::Internal => Error::InternalV2,
            services::ErrorKind::Forbidden => Error::Forbidden,
            services::ErrorKind::NotFound => Error::NotFound,
            services::ErrorKind::Validation(value) => Error::ValidateV2(value),
        }
    }
}

impl Codeable for Error {
    fn code(&self) -> StatusCode {
        match *self {
            Error::NotFound => StatusCode::NotFound,
            Error::Validate(_) => StatusCode::UnprocessableEntity,
            Error::ValidateV2(_) => StatusCode::UnprocessableEntity,
            Error::Parse => StatusCode::BadRequest,
            Error::Connection | Error::HttpClient | Error::InternalV2 => StatusCode::InternalServerError,
            Error::Forbidden | Error::InvalidToken => StatusCode::Forbidden,
        }
    }
}

impl PayloadCarrier for Error {
    fn payload(&self) -> Option<serde_json::Value> {
        match *self {
            Error::Validate(ref e) => serde_json::to_value(e.clone()).ok(),
            Error::ValidateV2(ref e) => Some(e.clone()),
            _ => None,
        }
    }
}
