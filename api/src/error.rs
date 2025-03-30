use std::path::PathBuf;

use axum::extract::rejection::JsonRejection;
use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};
use deadpool_diesel::{InteractError, PoolError};
use memo::role::{InvalidPermissionsError, InvalidRolesError};
use serde::{Deserialize, Serialize};
use snafu::{Backtrace, Snafu};

pub type Result2<T> = std::result::Result<T, Error2>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error2 {
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
impl From<&str> for Error2 {
    fn from(val: &str) -> Self {
        Self::Whatever {
            msg: val.to_string(),
        }
    }
}

impl From<String> for Error2 {
    fn from(val: String) -> Self {
        Self::Whatever { msg: val }
    }
}

/// Allow Error to be converted to StatusCode
impl From<Error2> for StatusCode {
    fn from(err: Error2) -> Self {
        match err {
            Error2::Validation { .. } => StatusCode::BAD_REQUEST,
            Error2::MaxClientsReached => StatusCode::BAD_REQUEST,
            Error2::MaxUsersReached => StatusCode::BAD_REQUEST,
            Error2::MaxBucketsReached => StatusCode::BAD_REQUEST,
            Error2::MaxDirsReached => StatusCode::BAD_REQUEST,
            Error2::MaxFilesReached => StatusCode::BAD_REQUEST,
            Error2::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Error2::Forbidden { .. } => StatusCode::FORBIDDEN,
            Error2::JsonRejection { .. } => StatusCode::BAD_REQUEST,
            Error2::MissingUploadFile { .. } => StatusCode::BAD_REQUEST,
            Error2::CreateFile { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error2::FileTypeNotAllowed => StatusCode::BAD_REQUEST,
            Error2::NotFound { .. } => StatusCode::NOT_FOUND,
            Error2::InvalidAuthToken => StatusCode::UNAUTHORIZED,
            Error2::InsufficientAuthScope => StatusCode::UNAUTHORIZED,
            Error2::NoAuthToken => StatusCode::UNAUTHORIZED,
            Error2::InvalidClient => StatusCode::UNAUTHORIZED,
            Error2::RequiresAuth => StatusCode::UNAUTHORIZED,
            Error2::InvalidPassword => StatusCode::UNAUTHORIZED,
            Error2::InactiveUser => StatusCode::UNAUTHORIZED,
            Error2::UserNotFound => StatusCode::UNAUTHORIZED,
            Error2::InvalidRoles { .. } => StatusCode::BAD_REQUEST,
            Error2::InvalidPermissions { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Allow errors to be rendered as response
impl IntoResponse for Error2 {
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

pub fn to_json_error_response(error: Error2) -> Response<Body> {
    match error {
        Error2::Validation { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error2::MaxClientsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of clients reached: 10",
            "Bad Request",
        ),
        Error2::MaxUsersReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of users reached: 100",
            "Bad Request",
        ),
        Error2::MaxBucketsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of buckets reached: 50",
            "Bad Request",
        ),
        Error2::MaxDirsReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of directories reached: 1000",
            "Bad Request",
        ),
        Error2::MaxFilesReached => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Maximum number of files reached: 1000",
            "Bad Request",
        ),
        Error2::Google { msg } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            msg.as_str(),
            "Internal Server Error",
        ),
        Error2::BadRequest { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error2::Forbidden { msg } => {
            create_json_error_response(StatusCode::FORBIDDEN, msg.as_str(), "Forbidden")
        }
        Error2::JsonRejection { msg, .. } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error2::MissingUploadFile { msg } => {
            create_json_error_response(StatusCode::BAD_REQUEST, msg.as_str(), "Bad Request")
        }
        Error2::CreateFile { .. } => create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unable to create file").as_str(),
            "Internal Server Error",
        ),
        Error2::FileTypeNotAllowed => create_json_error_response(
            StatusCode::BAD_REQUEST,
            "File type not allowed",
            "Bad Request",
        ),
        Error2::NotFound { msg } => {
            create_json_error_response(StatusCode::NOT_FOUND, msg.as_str(), "Not Found")
        }
        Error2::InvalidAuthToken => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid auth token",
            "Unauthorized",
        ),
        Error2::InsufficientAuthScope => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Insufficient auth scope",
            "Unauthorized",
        ),
        Error2::NoAuthToken => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "No auth token", "Unauthorized")
        }
        Error2::InvalidClient => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Invalid client", "Unauthorized")
        }
        Error2::RequiresAuth => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Requires authentication",
            "Unauthorized",
        ),
        Error2::InvalidPassword => create_json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
            "Unauthorized",
        ),
        Error2::InactiveUser => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "Inactive user", "Unauthorized")
        }
        Error2::UserNotFound => {
            create_json_error_response(StatusCode::UNAUTHORIZED, "User not found", "Unauthorized")
        }
        Error2::InvalidRoles { .. } => {
            create_json_error_response(StatusCode::BAD_REQUEST, "Invalid roles", "Bad Request")
        }
        Error2::InvalidPermissions { .. } => create_json_error_response(
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
