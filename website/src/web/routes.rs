use axum::extract::State;
use axum::handler::HandlerWithoutStateExt;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, get_service, post};
use axum::{Extension, Json, Router, middleware};
use reqwest::StatusCode;
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use tracing::error;

use crate::ctx::Ctx;
use crate::error::{ErrorInfo, ErrorMessageDto};
use crate::models::Pref;
use crate::run::AppState;
use crate::web::auth::auth_callback_handler;
use crate::web::files::{add_file_handler, generate_upload_url_handler};
use crate::web::login::{login_handler, login_page_handler};
use crate::web::middleware::dir_type_middleware;
use crate::web::{error_handler, index_handler, logout_handler};

use super::dirs::{
    dir_page_handler, edit_dir_controls_handler, edit_dir_handler, get_delete_dir_handler,
    new_dir_handler, post_delete_dir_handler, post_edit_dir_handler, post_new_dir_handler,
    search_dirs_handler,
};
use super::files::{
    confirm_delete_file_handler, document_listing_handler, exec_delete_file_handler,
    file_actions_handler, photo_listing_handler, upload_page_handler,
};
use super::middleware::{
    auth_middleware, dir_middleware, file_middleware, pref_middleware, require_auth_middleware,
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
        .nest("/{dir_type}", dir_routes(state.clone()))
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

fn dir_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(my_bucket_page_handler))
        .route("/search_dirs", get(search_dirs_handler))
        .route("/new_dir", get(new_dir_handler).post(post_new_dir_handler))
        .nest("/{dir_id}", dir_inner_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            dir_type_middleware,
        ))
        .with_state(state)
}

fn dir_inner_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(dir_page_handler))
        .route("/edit_controls", get(edit_dir_controls_handler))
        .route("/edit", get(edit_dir_handler).post(post_edit_dir_handler))
        .route(
            "/delete",
            get(get_delete_dir_handler).post(post_delete_dir_handler),
        )
        .route("/photo_grid", get(photo_listing_handler))
        .route("/file_table", get(document_listing_handler))
        .nest("/upload-url", upload_api_routes(state.clone()))
        .nest("/upload", upload_route(state.clone()))
        .nest("/files/{file_id}", file_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            dir_middleware,
        ))
        .with_state(state)
}

pub fn upload_api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(generate_upload_url_handler))
        .layer(middleware::map_response_with_state(
            state.clone(),
            api_response_mapper,
        ))
        .with_state(state)
}

fn upload_route(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(upload_page_handler).post(add_file_handler))
        .with_state(state)
}

fn file_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/delete",
            get(confirm_delete_file_handler).post(exec_delete_file_handler),
        )
        .route("/file-actions", get(file_actions_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            file_middleware,
        ))
        .with_state(state)
}

pub fn public_routes(state: AppState) -> Router {
    Router::new()
        .route("/auth/callback", get(auth_callback_handler))
        .route("/login", get(login_page_handler).post(login_handler))
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
        let actor = ctx.actor();
        return handle_error(&state, actor, &pref, e.clone(), full_page);
    }
    res
}

async fn api_response_mapper(res: Response) -> Response {
    let error = res.extensions().get::<ErrorInfo>();
    if let Some(e) = error {
        if e.status_code.is_server_error() {
            // Build the error response
            error!("{}", e.message);
        }

        let error_message = ErrorMessageDto {
            status_code: e.status_code.as_u16(),
            message: e.message.clone(),
            error: e.status_code.canonical_reason().unwrap().to_string(),
        };

        return (e.status_code, Json(error_message)).into_response();
    }
    res
}
