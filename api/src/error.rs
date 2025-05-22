use std::path::PathBuf;

use axum::extract::rejection::JsonRejection;
use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use deadpool_diesel::{InteractError, PoolError};
use memo::role::{InvalidPermissionsError, InvalidRolesError};
use serde::{Deserialize, Serialize};
use snafu::{Backtrace, ErrorCompat, Snafu};

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

    #[snafu(display("Error getting db connection: {}", source))]
    DbPool {
        source: PoolError,
        backtrace: Backtrace,
    },

    #[snafu(display("Error using the db connection: {}", source))]
    DbInteract {
        source: InteractError,
        backtrace: Backtrace,
    },

    #[snafu(display("Error querying {}: {}", table, source))]
    DbQuery {
        table: String,
        source: diesel::result::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("{} - {}", msg, source))]
    PasswordPrompt {
        msg: String,
        source: std::io::Error,
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

    #[snafu(display("Google Cloud error: {}", msg))]
    Google { msg: String },

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

    #[snafu(display("{}", msg))]
    HashPassword { msg: String },

    #[snafu(display("{}", msg))]
    VerifyPasswordHash { msg: String },

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

    #[snafu(display("Upload error: {}", source))]
    UploadFile {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Exif error: {}", source))]
    ExifInfo {
        source: exif::Error,
        backtrace: Backtrace,
    },

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
            Error::UserNotFound => StatusCode::UNAUTHORIZED,
            Error::InvalidRoles { .. } => StatusCode::BAD_REQUEST,
            Error::InvalidPermissions { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let status_code = StatusCode::from(&self);
        let message = format!("{}", self);
        let mut backtrace: Option<String> = None;
        if let Some(bt) = ErrorCompat::backtrace(&self) {
            backtrace = Some(format!("{}", bt));
        }

        // Build a dummy response
        let mut res = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(ErrorInfo {
            status_code,
            message,
            backtrace,
        });

        res
    }
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub status_code: StatusCode,
    pub message: String,
    pub backtrace: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse<'a> {
    pub status_code: u16,
    pub message: &'a str,
    pub error: &'a str,
}
