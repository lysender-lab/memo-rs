use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
};
use tower_http::limit::RequestBodyLimitLayer;

use super::{
    handler::{
        create_dir_handler, create_file_handler, delete_dir_handler, delete_file_handler,
        get_bucket_handler, get_dir_handler, get_file_handler, health_live_handler,
        health_ready_handler, home_handler, list_buckets_handler, list_dirs_handler,
        list_files_handler, not_found_handler, update_dir_handler,
    },
    middleware::{
        auth_middleware, bucket_middleware, dir_middleware, file_middleware,
        require_auth_middleware,
    },
};
use crate::{
    state::AppState,
    web::handler::{oauth_profile_handler, oauth_token_handler, update_bucket_handler},
};

pub fn all_routes(state: AppState) -> Router {
    Router::new()
        .merge(public_routes(state.clone()))
        .merge(private_routes(state.clone()))
        .fallback(any(not_found_handler))
        .with_state(state)
}

fn public_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(home_handler))
        .route("/health/liveness", get(health_live_handler))
        .route("/health/readiness", get(health_ready_handler))
        .route("/oauth/token", post(oauth_token_handler))
        .route("/oauth/profile", post(oauth_profile_handler))
        .with_state(state)
}

fn private_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/buckets", buckets_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

fn buckets_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_buckets_handler))
        .nest("/{bucket_id}", inner_bucket_routes(state.clone()))
        .with_state(state)
}

fn inner_bucket_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_bucket_handler).patch(update_bucket_handler))
        .nest("/dirs", dir_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bucket_middleware,
        ))
        .with_state(state)
}

fn dir_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_dirs_handler).post(create_dir_handler))
        .nest("/{dir_id}", inner_dir_routes(state.clone()))
        .with_state(state)
}

fn inner_dir_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(get_dir_handler)
                .patch(update_dir_handler)
                .delete(delete_dir_handler),
        )
        .nest("/files", files_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            dir_middleware,
        ))
        .with_state(state)
}

fn files_routes(state: AppState) -> Router<AppState> {
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
