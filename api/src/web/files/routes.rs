use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::{Router, routing::get};
use tower_http::limit::RequestBodyLimitLayer;

use crate::web::middlewares::file_middleware;
use crate::web::server::AppState;

use super::{create_file_handler, delete_file_handler, get_file_handler, list_files_handler};

pub fn files_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_files_handler).post(create_file_handler))
        .nest("/{file_id}", inner_file_routes(state.clone()))
        .layer(DefaultBodyLimit::max(8000000))
        .layer(RequestBodyLimitLayer::new(8000000))
        .with_state(state)
}

fn inner_file_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_file_handler).delete(delete_file_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            file_middleware,
        ))
        .with_state(state)
}
