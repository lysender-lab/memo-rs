use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use derive_more::From;
use serde::Deserialize;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, From)]
pub enum Error {
    #[from]
    AnyError(String),
    ValidationError(String),
    BadRequest(String),
    Forbidden(String),
    LoginFailed(String),
    InvalidCaptcha(String),
    CaptchaResponseError(String),
    LoginRequired(String),
    NoDefaultBucket,
    AlbumNotFound,
    PhotoNotFound,
    NoAuthCookie,
    InvalidCsrfToken,
    JsonParseError(String),
    ServiceError(String),
}

#[derive(Deserialize)]
pub struct ErrorResponse {
    #[allow(dead_code)]
    pub status_code: u16,

    pub message: String,

    #[allow(dead_code)]
    pub error: String,
}

/// Allow string slices to be converted to Error
impl From<&str> for Error {
    fn from(val: &str) -> Self {
        Self::AnyError(val.to_string())
    }
}

/// Allow errors to be displayed as string
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::AnyError(val) => write!(f, "{}", val),
            Self::ValidationError(val) => write!(f, "{}", val),
            Self::BadRequest(val) => write!(f, "{}", val),
            Self::Forbidden(val) => write!(f, "{}", val),
            Self::LoginFailed(val) => write!(f, "{}", val),
            Self::InvalidCaptcha(val) => write!(f, "{}", val),
            Self::CaptchaResponseError(val) => write!(f, "{}", val),
            Self::LoginRequired(val) => write!(f, "{}", val),
            Self::NoDefaultBucket => write!(f, "No default bucket configured"),
            Self::AlbumNotFound => write!(f, "Album not found"),
            Self::PhotoNotFound => write!(f, "Photo not found"),
            Self::NoAuthCookie => write!(f, "Login to continue"),
            Self::InvalidCsrfToken => write!(f, "Stale form data. Refresh the page and try again"),
            Self::JsonParseError(val) => write!(f, "{}", val),
            Self::ServiceError(val) => write!(f, "{}", val),
        }
    }
}

/// Allow Error to be converted to StatusCode
impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::AnyError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ValidationError(_) => StatusCode::BAD_REQUEST,
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::Forbidden(_) => StatusCode::FORBIDDEN,
            Error::LoginFailed(_) => StatusCode::UNAUTHORIZED,
            Error::InvalidCaptcha(_) => StatusCode::BAD_REQUEST,
            Error::CaptchaResponseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::LoginRequired(_) => StatusCode::UNAUTHORIZED,
            Error::NoDefaultBucket => StatusCode::INTERNAL_SERVER_ERROR,
            Error::AlbumNotFound => StatusCode::NOT_FOUND,
            Error::PhotoNotFound => StatusCode::NOT_FOUND,
            Error::NoAuthCookie => StatusCode::UNAUTHORIZED,
            Error::InvalidCsrfToken => StatusCode::BAD_REQUEST,
            Error::JsonParseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ServiceError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        StatusCode::from(self).into_response()
    }
}
