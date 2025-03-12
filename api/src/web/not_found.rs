use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::error::ErrorResponse;
use super::server::AppState;

pub async fn not_found_handler(State(_state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            status_code: StatusCode::NOT_FOUND.as_u16(),
            message: "Not Found".to_string(),
            error: "Not Found".to_string(),
        }),
    )
}
