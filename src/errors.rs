use hyper::StatusCode;
use serde_json;
use validator::ValidationErrors;

use stq_http::errors::{Codeable, PayloadCarrier};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Not found")]
    NotFound,
    #[fail(display = "Parse error")]
    Parse,
    #[fail(display = "Validation error")]
    Validate(ValidationErrors),
    #[fail(display = "Server is refusing to fullfil the reqeust")]
    Forbidden,
    #[fail(display = "R2D2 connection error")]
    Connection,
    #[fail(display = "Http Client error")]
    HttpClient,
    #[fail(display = "Invalid oauth token")]
    InvalidToken,
}

impl Codeable for Error {
    fn code(&self) -> StatusCode {
        match *self {
            Error::NotFound => StatusCode::NotFound,
            Error::Validate(_) => StatusCode::BadRequest,
            Error::Parse => StatusCode::UnprocessableEntity,
            Error::Connection | Error::HttpClient => StatusCode::InternalServerError,
            Error::Forbidden | Error::InvalidToken => StatusCode::Forbidden,
        }
    }
}

impl PayloadCarrier for Error {
    fn payload(&self) -> Option<serde_json::Value> {
        match *self {
            Error::Validate(ref e) => serde_json::to_value(e.clone()).ok(),
            _ => None,
        }
    }
}
