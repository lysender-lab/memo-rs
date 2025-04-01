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
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let message = format!("{}", self);
        let mut backtrace: Option<String> = None;
        if let Some(bt) = ErrorCompat::backtrace(&self) {
            backtrace = Some(format!("{}", bt));
        }
        let mut res = to_json_error_response(self);
        res.extensions_mut()
            .insert(ErrorInfo { message, backtrace });
        res
    }
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub message: String,
    pub backtrace: Option<String>,
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
        Error::Google { msg } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            msg.as_str(),
            "Internal Server Error",
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
        _ => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Internal Server Error",
        ),
    }
}
