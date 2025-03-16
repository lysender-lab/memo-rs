use crate::web::server::AppState;

use axum::{
    Router, middleware,
    routing::{any, get, post},
};

use super::{
    auth::{authenticate_handler, user_routes},
    buckets::buckets_routes,
    health::{health_live_handler, health_ready_handler},
    home::home_handler,
    middlewares::auth_middleware,
    not_found::not_found_handler,
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
        .route("/v1/auth/token", post(authenticate_handler))
        .with_state(state)
}

fn private_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/v1/buckets", buckets_routes(state.clone()))
        .nest("/v1/user", user_routes(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}
