use std::path::PathBuf;

use axum::extract::rejection::JsonRejection;
use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use memo::role::{InvalidPermissionsError, InvalidRolesError};
use serde::{Deserialize, Serialize};
use snafu::{Backtrace, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Error reading config file: {}", source))]
    ConfigFile {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Error parsing config file: {}", source))]
    ConfigParse {
        source: toml::de::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Unable to create upload dir: {}", source))]
    UploadDir {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Config error: {}", msg))]
    Config { msg: String },

    #[snafu(display("Failed to read bundles.json: {}", source))]
    ManifestRead {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to parse bundles.json: {}", source))]
    ManifestParse {
        source: serde_json::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("{}", msg))]
    Validation { msg: String },

    #[snafu(display("Maximum number of clients reached: 10"))]
    MaxClientsReached,

    #[snafu(display("Maximum number of users reached: 100"))]
    MaxUsersReached,

    #[snafu(display("Maximum number of buckets reached: 50"))]
    MaxBucketsReached,

    #[snafu(display("Maximum number of directories reached: 1000"))]
    MaxDirsReached,

    #[snafu(display("Maximum number of files reached: 1000"))]
    MaxFilesReached,

    #[snafu(display("{}", msg))]
    BadRequest { msg: String },

    #[snafu(display("{}", msg))]
    Forbidden { msg: String },

    #[snafu(display("{}", msg))]
    JsonRejection {
        msg: String,
        source: JsonRejection,
        backtrace: Backtrace,
    },

    #[snafu(display("{}", msg))]
    MissingUploadFile { msg: String },

    #[snafu(display("Unable to create file: {:?}", path))]
    CreateFile {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("File type not allowed"))]
    FileTypeNotAllowed,

    #[snafu(display("{}", msg))]
    NotFound { msg: String },

    #[snafu(display("Invalid auth token"))]
    InvalidAuthToken,

    #[snafu(display("Insufficient auth scope"))]
    InsufficientAuthScope,

    #[snafu(display("No auth token"))]
    NoAuthToken,

    #[snafu(display("Invalid client"))]
    InvalidClient,

    #[snafu(display("Requires authentication"))]
    RequiresAuth,

    #[snafu(display("Invalid username or password"))]
    InvalidPassword,

    #[snafu(display("Inactive user"))]
    InactiveUser,

    #[snafu(display("User not found"))]
    UserNotFound,

    #[snafu(display("{}", source))]
    InvalidRoles {
        source: InvalidRolesError,
        backtrace: Backtrace,
    },

    #[snafu(display("{}", source))]
    InvalidPermissions {
        source: InvalidPermissionsError,
        backtrace: Backtrace,
    },

    #[snafu(display("{}: {}", msg, source))]
    HttpClient {
        msg: String,
        source: reqwest::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("{}: {}", msg, source))]
    HttpResponseParse {
        msg: String,
        source: reqwest::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid username or password"))]
    LoginFailed,

    #[snafu(display("Login to continue"))]
    LoginRequired,

    #[snafu(display("{}", msg))]
    Service { msg: String },

    #[snafu(display("Album not found"))]
    AlbumNotFound,

    #[snafu(display("Stale form data. Refresh the page and try again."))]
    CsrfToken,

    #[snafu(display("{}", msg))]
    Whatever { msg: String },
}

// Allow string slices to be converted to Error
impl From<&str> for Error {
    fn from(val: &str) -> Self {
        Self::Whatever {
            msg: val.to_string(),
        }
    }
}

impl From<String> for Error {
    fn from(val: String) -> Self {
        Self::Whatever { msg: val }
    }
}

/// Allow Error to be converted to StatusCode
impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Validation { .. } => StatusCode::BAD_REQUEST,
            Error::MaxClientsReached => StatusCode::BAD_REQUEST,
            Error::MaxUsersReached => StatusCode::BAD_REQUEST,
            Error::MaxBucketsReached => StatusCode::BAD_REQUEST,
            Error::MaxDirsReached => StatusCode::BAD_REQUEST,
            Error::MaxFilesReached => StatusCode::BAD_REQUEST,
            Error::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Error::Forbidden { .. } => StatusCode::FORBIDDEN,
            Error::JsonRejection { .. } => StatusCode::BAD_REQUEST,
            Error::MissingUploadFile { .. } => StatusCode::BAD_REQUEST,
            Error::CreateFile { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::FileTypeNotAllowed => StatusCode::BAD_REQUEST,
            Error::NotFound { .. } => StatusCode::NOT_FOUND,
            Error::InvalidAuthToken => StatusCode::UNAUTHORIZED,
            Error::InsufficientAuthScope => StatusCode::UNAUTHORIZED,
            Error::NoAuthToken => StatusCode::UNAUTHORIZED,
            Error::InvalidClient => StatusCode::UNAUTHORIZED,
            Error::RequiresAuth => StatusCode::UNAUTHORIZED,
            Error::InvalidPassword => StatusCode::UNAUTHORIZED,
            Error::InactiveUser => StatusCode::UNAUTHORIZED,
            Error::UserNotFound => StatusCode::UNAUTHORIZED,
            Error::InvalidRoles { .. } => StatusCode::BAD_REQUEST,
            Error::InvalidPermissions { .. } => StatusCode::BAD_REQUEST,
            Error::LoginFailed { .. } => StatusCode::UNAUTHORIZED,
            Error::LoginRequired => StatusCode::UNAUTHORIZED,
            Error::AlbumNotFound => StatusCode::NOT_FOUND,
            Error::CsrfToken => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
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
pub struct ErrorResponse {
    pub status_code: u16,
    pub message: String,
    pub error: String,
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
        message: message.to_string(),
        error: error.to_string(),
    };

    return create_json_response(status, serde_json::to_string(&body).unwrap());
}

pub fn to_json_error_response(error: Error) -> Response<Body> {
    match error {
        Error::Validation { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error::MaxClientsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of clients reached: 10",
            "Bad Request",
        ),
        Error::MaxUsersReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of users reached: 100",
            "Bad Request",
        ),
        Error::MaxBucketsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of buckets reached: 50",
            "Bad Request",
        ),
        Error::MaxDirsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of directories reached: 1000",
            "Bad Request",
        ),
        Error::MaxFilesReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of files reached: 1000",
            "Bad Request",
        ),
        Error::BadRequest { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error::Forbidden { msg } => {
            create_json_error_response(StatusCode::FORBIDDEN, msg.as_str(), "Forbidden")
        }
        Error::JsonRejection { msg, .. } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error::MissingUploadFile { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error::CreateFile { .. } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unable to create file").as_str(),
            "Internal Server Error",
        ),
        Error::FileTypeNotAllowed => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "File type not allowed",
            "Bad Request",
        ),
        Error::NotFound { msg } => {
            create_json_error_response(StatusCode::NOT_FOUND, msg.as_str(), "Not Found")
        }
        Error::InvalidAuthToken => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid auth token",
            "Unauthorized",
        ),
        Error::InsufficientAuthScope => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Insufficient auth scope",
            "Unauthorized",
        ),
        Error::NoAuthToken => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "No auth token", "Unauthorized")
        }
        Error::InvalidClient => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Invalid client", "Unauthorized")
        }
        Error::RequiresAuth => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Requires authentication",
            "Unauthorized",
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
            create_json_error_response(StatusCode::UNAUTHORIZED, "User not found", "Unauthorized")
        }
        Error::InvalidRoles { .. } => {
            create_json_error_response(StatusCode::BAD_REQUEST, "Invalid roles", "Bad Request")
        }
        Error::InvalidPermissions { .. } => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid permissions",
            "Bad Request",
        ),
        Error::HttpClient { msg, .. } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            msg.as_str(),
            "Internal Server Error",
        ),
        Error::HttpResponseParse { msg, .. } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            msg.as_str(),
            "Internal Server Error",
        ),
        Error::LoginFailed => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
            "Unauthorized",
        ),
        Error::LoginRequired => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Login to continue",
            "Unauthorized",
        ),
        Error::Service { msg } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            msg.as_str(),
            "Internal Server Error",
        ),
        Error::AlbumNotFound => {
            create_json_error_response(StatusCode::NOT_FOUND, "Album not found", "Not Found")
        }
        Error::CsrfToken => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Stale form data. Refresh the page and try again.",
            "Bad Request",
        ),
        _ => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Internal Server Error",
        ),
    }
}
