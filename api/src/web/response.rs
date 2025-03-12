use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};

use crate::Error;
use crate::web::error::ErrorResponse;

#[derive(Debug)]
pub struct JsonResponse {
    pub status_code: StatusCode,
    pub data: String,
}

impl JsonResponse {
    pub fn new(data: String) -> Self {
        JsonResponse {
            status_code: StatusCode::OK,
            data,
        }
    }

    pub fn with_status(status_code: StatusCode, data: String) -> Self {
        JsonResponse { status_code, data }
    }
}

impl IntoResponse for JsonResponse {
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(self.status_code)
            .header("Content-Type", "application/json")
            .body(Body::from(self.data))
            .unwrap()
    }
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
