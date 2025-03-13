use axum::response::IntoResponse;
use axum::{body::Body, http::StatusCode, response::Response};

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
