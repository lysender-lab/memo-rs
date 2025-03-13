use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use derive_more::From;
use serde::Serialize;

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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub status_code: u16,
    pub message: String,
    pub error: String,
}

pub fn create_response(status: StatusCode, body: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

pub fn create_error_response(status: StatusCode, message: String, error: String) -> Response<Body> {
    let body = ErrorResponse {
        status_code: status.as_u16(),
        message,
        error,
    };

    return create_response(status, serde_json::to_string(&body).unwrap());
}

pub fn to_error_response(error: Error) -> Response<Body> {
    match error {
        Error::AnyError(message) => create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message,
            "Internal Server Error".to_string(),
        ),
        Error::BadRequest(message) => {
            create_error_response(StatusCode::BAD_REQUEST, message, "Bad Request".to_string())
        }
        Error::Forbidden(message) => {
            create_error_response(StatusCode::FORBIDDEN, message, "Forbidden".to_string())
        }
        Error::ValidationError(message) => {
            create_error_response(StatusCode::BAD_REQUEST, message, "Bad Request".to_string())
        }
        Error::MissingUploadFile(message) => {
            create_error_response(StatusCode::BAD_REQUEST, message, "Bad Request".to_string())
        }
        Error::FileTypeNotAllowed => create_error_response(
            StatusCode::BAD_REQUEST,
            "File type not allowed".to_string(),
            "Bad Request".to_string(),
        ),
        Error::NotFound(message) => {
            create_error_response(StatusCode::NOT_FOUND, message, "Not Found".to_string())
        }
        Error::InvalidAuthToken => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::InsufficientAuthScope => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::NoAuthToken => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::InvalidClient => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::RequiresAuth => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::HashPasswordError(message) => create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message,
            "Internal Server Error".to_string(),
        ),
        Error::VerifyPasswordHashError(message) => create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message,
            "Internal Server Error".to_string(),
        ),
        Error::InvalidPassword => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::InactiveUser => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Inactive user".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::UserNotFound => create_error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
            "Unauthorized".to_string(),
        ),
        Error::ConfigError(message) => create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message,
            "Internal Server Error".to_string(),
        ),
    }
}
