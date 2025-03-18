use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use derive_more::From;
use serde::{Deserialize, Serialize};

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

    // Website errors
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

            // Website errors
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
            Error::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            // Website errors
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
        Error::ConfigError(message) => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            message.as_str(),
            "Internal Server Error",
        ),

        // Website errors are not handled here
        _ => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Internal Server Error",
        ),
    }
}
