use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use derive_more::From;

use crate::web::response::to_error_response;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, From)]
pub enum Error {
    #[from]
    AnyError(String),
    BadRequest(String),
    Forbidden(String),
    ValidationError(String),
    MissingUploadFile(String),
    FileTypeNotAllowed,
    NotFound(String),
    InvalidAuthToken,
    InsufficientAuthScope,
    NoAuthToken,
    InvalidClient,
    RequiresAuth,
    HashPasswordError(String),
    VerifyPasswordHashError(String),
    InvalidPassword,
    InactiveUser,
    UserNotFound,
    ConfigError(String),
}

// Allow string slices to be converted to Error
impl From<&str> for Error {
    fn from(val: &str) -> Self {
        Self::AnyError(val.to_string())
    }
}

// Allow errors to be displayed as string
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::AnyError(val) => write!(f, "{}", val),
            Self::BadRequest(val) => write!(f, "{}", val),
            Self::Forbidden(val) => write!(f, "{}", val),
            Self::ValidationError(val) => write!(f, "{}", val),
            Self::MissingUploadFile(val) => write!(f, "{}", val),
            Self::FileTypeNotAllowed => write!(f, "{}", "File type not allowed"),
            Self::NotFound(val) => write!(f, "{}", val),
            Self::InvalidAuthToken => write!(f, "Invalid auth token"),
            Self::InsufficientAuthScope => write!(f, "Insufficient auth scope"),
            Self::NoAuthToken => write!(f, "No auth token"),
            Self::InvalidClient => write!(f, "Invalid client"),
            Self::RequiresAuth => write!(f, "Requires authentication"),
            Self::HashPasswordError(val) => write!(f, "{}", val),
            Self::VerifyPasswordHashError(val) => write!(f, "{}", val),
            Self::InvalidPassword => write!(f, "Invalid password"),
            Self::InactiveUser => write!(f, "Inactive user"),
            Self::UserNotFound => write!(f, "User not found"),
            Self::ConfigError(val) => write!(f, "{}", val),
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        to_error_response(self)
    }
}
