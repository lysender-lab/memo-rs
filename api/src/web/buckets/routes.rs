use axum::{Router, middleware, routing::get};

use crate::web::{
    dirs::dir_routes,
    middlewares::{bucket_middleware, require_auth_middleware},
    server::AppState,
};

use super::handlers::{get_bucket_handler, list_buckets_handler};

pub fn buckets_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_buckets_handler))
        .nest("/{bucket_id}", inner_bucket_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_middleware,
        ))
        .with_state(state)
}

fn inner_bucket_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_bucket_handler))
        .nest("/dirs", dir_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bucket_middleware,
        ))
        .with_state(state)
}
