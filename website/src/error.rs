use axum::http::{self, StatusCode};
use axum::{
    body::Body,
    extract::rejection::JsonRejection,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use snafu::{Backtrace, ErrorCompat, Snafu};
use std::path::PathBuf;

use memo::role::{InvalidPermissionsError, InvalidRolesError};

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

    #[snafu(display("Failed to render template: {}", source))]
    Template {
        source: askama::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Response builder error: {}", source))]
    ResponseBuilder {
        source: http::Error,
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

    #[snafu(display("File not found"))]
    FileNotFound,

    #[snafu(display("Album not found"))]
    AlbumNotFound,

    #[snafu(display("Bucket not found"))]
    BucketNotFound,

    #[snafu(display("Client not found"))]
    ClientNotFound,

    #[snafu(display("Stale form data. Refresh the page and try again."))]
    CsrfToken,

    #[snafu(display("Failed to initialize form data. Refresh the page and try again."))]
    CsrfInit,

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
impl From<&Error> for StatusCode {
    fn from(err: &Error) -> Self {
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
            Error::UserNotFound => StatusCode::NOT_FOUND,
            Error::InvalidRoles { .. } => StatusCode::BAD_REQUEST,
            Error::InvalidPermissions { .. } => StatusCode::BAD_REQUEST,
            Error::LoginFailed { .. } => StatusCode::UNAUTHORIZED,
            Error::LoginRequired => StatusCode::UNAUTHORIZED,
            Error::FileNotFound => StatusCode::NOT_FOUND,
            Error::AlbumNotFound => StatusCode::NOT_FOUND,
            Error::BucketNotFound => StatusCode::NOT_FOUND,
            Error::ClientNotFound => StatusCode::NOT_FOUND,
            Error::CsrfToken => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let status_code = StatusCode::from(&self);
        let title = status_code
            .canonical_reason()
            .expect("status_code must be valid")
            .to_string();
        let message = format!("{}", self);
        let mut backtrace: Option<String> = None;
        if let Some(bt) = ErrorCompat::backtrace(&self) {
            backtrace = Some(format!("{}", bt));
        }

        // Build a dummy response
        let mut res = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .expect("Response builder must succeed");

        res.extensions_mut().insert(ErrorInfo {
            status_code,
            title,
            message,
            backtrace,
        });

        res
    }
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status_code: u16,
    pub message: String,
    pub error: String,
}

#[derive(Clone)]
pub struct ErrorInfo {
    pub status_code: StatusCode,
    pub title: String,
    pub message: String,
    pub backtrace: Option<String>,
}

impl From<&Error> for ErrorInfo {
    fn from(e: &Error) -> Self {
        let status_code = e.into();
        let msg = e.to_string();
        Self {
            status_code,
            title: status_code
                .canonical_reason()
                .expect("status_code must be valid")
                .to_string(),
            message: msg,
            backtrace: None,
        }
    }
}
