use crate::config::ConfigError;
use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use serde::{Deserialize, Serialize};
use snafu::{Backtrace, Snafu};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub type Result2<T> = std::result::Result<T, Error2>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error2 {
    #[snafu(display("{}", source))]
    Config {
        source: ConfigError,
        backtrace: Backtrace,
    },
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    AnyError(String),

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    ValidationError(String),

    #[error("{0}")]
    MissingUploadFile(String),

    #[error("File type not allowed")]
    FileTypeNotAllowed,

    #[error("{0}")]
    NotFound(String),

    #[error("Invalid auth token")]
    InvalidAuthToken,

    #[error("Insufficient auth scope")]
    InsufficientAuthScope,

    #[error("No auth token")]
    NoAuthToken,

    #[error("Invalid client")]
    InvalidClient,

    #[error("Requires authentication")]
    RequiresAuth,

    #[error("{0}")]
    HashPasswordError(String),

    #[error("{0}")]
    VerifyPasswordHashError(String),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Inactive user")]
    InactiveUser,

    #[error("User not found")]
    UserNotFound,
}

// Allow string slices to be converted to Error
impl From<&str> for Error {
    fn from(val: &str) -> Self {
        Self::AnyError(val.to_string())
    }
}

impl From<String> for Error {
    fn from(val: String) -> Self {
        Self::AnyError(val)
    }
}

/// Allow Error to be converted to StatusCode
impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::AnyError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::Forbidden(_) => StatusCode::FORBIDDEN,
            Error::ValidationError(_) => StatusCode::BAD_REQUEST,
            Error::MissingUploadFile(_) => StatusCode::BAD_REQUEST,
            Error::FileTypeNotAllowed => StatusCode::BAD_REQUEST,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::InvalidAuthToken => StatusCode::UNAUTHORIZED,
            Error::InsufficientAuthScope => StatusCode::UNAUTHORIZED,
            Error::NoAuthToken => StatusCode::UNAUTHORIZED,
            Error::InvalidClient => StatusCode::UNAUTHORIZED,
            Error::RequiresAuth => StatusCode::UNAUTHORIZED,
            Error::HashPasswordError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::VerifyPasswordHashError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::InvalidPassword => StatusCode::UNAUTHORIZED,
            Error::InactiveUser => StatusCode::UNAUTHORIZED,
            Error::UserNotFound => StatusCode::UNAUTHORIZED,
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        to_json_error_response(self)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse<'a> {
    pub status_code: u16,
    pub message: &'a str,
    pub error: &'a str,
}

pub fn create_json_response(status: StatusCode, body: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

pub fn create_json_error_response(
    status: StatusCode,
    message: &str,
    error: &str,
) -> Response<Body> {
    let body = ErrorResponse {
        status_code: status.as_u16(),
        message,
        error,
    };

    return create_json_response(status, serde_json::to_string(&body).unwrap());
}

pub fn to_json_error_response(error: Error) -> Response<Body> {
    match error {
        Error::AnyError(message) => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message.as_str(),
            "Internal Server Error",
        ),
        Error::ConfigError(message) => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message.as_str(),
            "Internal Server Error",
        ),
        Error::BadRequest(message) => {
            create_json_error_response(StatusCode::BAD_REQUEST, message.as_str(), "Bad Request")
        }
        Error::Forbidden(message) => {
            create_json_error_response(StatusCode::FORBIDDEN, message.as_str(), "Forbidden")
        }
        Error::ValidationError(message) => {
            create_json_error_response(StatusCode::BAD_REQUEST, message.as_str(), "Bad Request")
        }
        Error::MissingUploadFile(message) => {
            create_json_error_response(StatusCode::BAD_REQUEST, message.as_str(), "Bad Request")
        }
        Error::FileTypeNotAllowed => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "File type not allowed",
            "Bad Request",
        ),
        Error::NotFound(message) => {
            create_json_error_response(StatusCode::NOT_FOUND, message.as_str(), "Not Found")
        }
        Error::InvalidAuthToken => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
        Error::InsufficientAuthScope => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
        Error::NoAuthToken => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
        Error::InvalidClient => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
        Error::RequiresAuth => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
        Error::HashPasswordError(message) => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message.as_str(),
            "Internal Server Error",
        ),
        Error::VerifyPasswordHashError(message) => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message.as_str(),
            "Internal Server Error",
        ),
        Error::InvalidPassword => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
            "Unauthorized",
        ),
        Error::InactiveUser => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Inactive user", "Unauthorized")
        }
        Error::UserNotFound => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Unauthorized", "Unauthorized")
        }
    }
}
