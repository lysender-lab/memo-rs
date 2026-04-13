use axum::extract::{DefaultBodyLimit, State};
use axum::handler::HandlerWithoutStateExt;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, get_service, post};
use axum::{Extension, Router, middleware};
use reqwest::StatusCode;
use std::path::Path;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::error;

use crate::ctx::Ctx;
use crate::error::ErrorInfo;
use crate::models::Pref;
use crate::run::AppState;
use crate::web::{error_handler, index_handler, login_handler, logout_handler, post_login_handler};

use super::dirs::{
    dir_page_handler, edit_dir_controls_handler, edit_dir_handler, get_delete_dir_handler,
    new_dir_handler, post_delete_dir_handler, post_edit_dir_handler, post_new_dir_handler,
    search_dirs_handler,
};
use super::files::{
    confirm_delete_photo_handler, exec_delete_photo_handler, photo_listing_v2_handler,
    pre_delete_photo_handler, upload_handler, upload_page_handler,
};
use super::middleware::{
    auth_middleware, dir_middleware, file_middleware, my_bucket_middleware, pref_middleware,
    require_auth_middleware,
};
use super::my_bucket::my_bucket_page_handler;
use super::profile::profile_page_handler;
use super::{dark_theme_handler, handle_error, light_theme_handler};

pub fn all_routes(state: AppState, frontend_dir: &Path) -> Router {
    Router::new()
        .merge(public_routes(state.clone()))
        .merge(private_routes(state.clone()))
        .merge(assets_routes(frontend_dir))
        .fallback(any(error_handler).with_state(state))
}

pub fn assets_routes(dir: &Path) -> Router {
    let target_dir = dir.join("public");
    Router::new()
        .route(
            "/manifest.json",
            get_service(ServeFile::new(target_dir.join("manifest.json"))),
        )
        .route(
            "/favicon.ico",
            get_service(ServeFile::new(target_dir.join("favicon.ico"))),
        )
        .nest_service(
            "/assets",
            get_service(
                ServeDir::new(target_dir.join("assets"))
                    .not_found_service(file_not_found.into_service()),
            ),
        )
}

async fn file_not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "File not found")
}

pub fn private_routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/prefs/theme/light", post(light_theme_handler))
        .route("/prefs/theme/dark", post(dark_theme_handler))
        .route("/profile", get(profile_page_handler))
        .nest("/buckets/{bucket_id}", my_bucket_routes(state.clone()))
        .layer(middleware::map_response_with_state(
            state.clone(),
            response_mapper,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .route_layer(middleware::from_fn(pref_middleware))
        .with_state(state)
}

fn my_bucket_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(my_bucket_page_handler))
        .route("/search_dirs", get(search_dirs_handler))
        .route("/new_dir", get(new_dir_handler).post(post_new_dir_handler))
        .nest("/dirs/{dir_id}", my_dir_inner_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            my_bucket_middleware,
        ))
        .with_state(state)
}

fn my_dir_inner_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(dir_page_handler))
        .route("/edit_controls", get(edit_dir_controls_handler))
        .route("/edit", get(edit_dir_handler).post(post_edit_dir_handler))
        .route(
            "/delete",
            get(get_delete_dir_handler).post(post_delete_dir_handler),
        )
        .route("/photo_grid", get(photo_listing_v2_handler))
        .nest("/upload", my_upload_route(state.clone()))
        .nest("/photos/{file_id}", my_photo_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            dir_middleware,
        ))
        .with_state(state)
}

fn my_upload_route(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(upload_page_handler).post(upload_handler))
        .layer(DefaultBodyLimit::max(8000000))
        .layer(RequestBodyLimitLayer::new(8000000))
        .with_state(state)
}

fn my_photo_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/delete",
            get(confirm_delete_photo_handler).post(exec_delete_photo_handler),
        )
        .route("/delete_controls", get(pre_delete_photo_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            file_middleware,
        ))
        .with_state(state)
}

pub fn public_routes(state: AppState) -> Router {
    Router::new()
        .route("/login", get(login_handler).post(post_login_handler))
        .route("/logout", post(logout_handler))
        .layer(middleware::map_response_with_state(
            state.clone(),
            response_mapper,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .route_layer(middleware::from_fn(pref_middleware))
        .with_state(state)
}

async fn response_mapper(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    headers: HeaderMap,
    res: Response,
) -> Response {
    let error = res.extensions().get::<ErrorInfo>();
    if let Some(e) = error {
        if e.status_code.is_server_error() {
            error!("{}", e.message);
        }

        let full_page = headers.get("HX-Request").is_none();
        let actor = ctx.actor().cloned();
        return handle_error(&state, actor, &pref, e.clone(), full_page);
    }
    res
}
