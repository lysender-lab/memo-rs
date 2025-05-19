use axum::extract::{DefaultBodyLimit, State};
use axum::handler::HandlerWithoutStateExt;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, get_service, post};
use axum::{Extension, Router, middleware};
use reqwest::StatusCode;
use std::path::PathBuf;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::error;

use crate::ctx::Ctx;
use crate::error::ErrorInfo;
use crate::models::Pref;
use crate::run::AppState;
use crate::web::{
    error_handler, index_handler, login_handler, logout_handler, new_album_handler,
    photo_listing_handler, photos_page_handler, post_login_handler, post_new_album_handler,
};

use super::clients::{
    client_page_handler, clients_handler, clients_listing_handler, edit_client_handler,
    new_client_handler, post_edit_client_handler, post_new_client_handler,
};
use super::middleware::{
    album_listing_middleware, album_middleware, auth_middleware, client_middleware,
    photo_middleware, pref_middleware, require_auth_middleware,
};
use super::users::users_handler;
use super::{
    album_listing_handler, confirm_delete_photo_handler, dark_theme_handler,
    edit_album_controls_handler, edit_album_handler, exec_delete_photo_handler,
    get_delete_album_handler, handle_error, light_theme_handler, post_delete_album_handler,
    post_edit_album_handler, pre_delete_photo_handler, upload_handler, upload_page_handler,
};

pub fn all_routes(state: AppState, frontend_dir: &PathBuf) -> Router {
    Router::new()
        .merge(public_routes(state.clone()))
        .merge(private_routes(state.clone()))
        .merge(assets_routes(frontend_dir))
        .fallback(any(error_handler).with_state(state))
}

pub fn assets_routes(dir: &PathBuf) -> Router {
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
        .nest("/albums", album_routes(state.clone()))
        .nest("/clients", client_routes(state.clone()))
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

fn album_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/listing", get(album_listing_handler))
        .route("/new", get(new_album_handler).post(post_new_album_handler))
        .nest("/{album_id}", album_inner_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            album_listing_middleware,
        ))
        .with_state(state)
}

fn album_inner_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(photos_page_handler))
        .route("/edit-controls", get(edit_album_controls_handler))
        .route(
            "/edit",
            get(edit_album_handler).post(post_edit_album_handler),
        )
        .route(
            "/delete",
            get(get_delete_album_handler).post(post_delete_album_handler),
        )
        .route("/photo-grid", get(photo_listing_handler))
        .nest("/upload", upload_route(state.clone()))
        .nest("/photos/{photo_id}", photo_routes(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            album_middleware,
        ))
        .with_state(state)
}

fn upload_route(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(upload_page_handler).post(upload_handler))
        .layer(DefaultBodyLimit::max(8000000))
        .layer(RequestBodyLimitLayer::new(8000000))
        .with_state(state)
}

fn photo_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/delete",
            get(confirm_delete_photo_handler).post(exec_delete_photo_handler),
        )
        .route("/delete-controls", get(pre_delete_photo_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            photo_middleware,
        ))
        .with_state(state)
}

fn client_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(clients_handler))
        .route("/listing", get(clients_listing_handler))
        .route(
            "/new",
            get(new_client_handler).post(post_new_client_handler),
        )
        .nest("/{client_id}", client_inner_routes(state.clone()))
        .with_state(state)
}

fn client_inner_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(client_page_handler))
        .route("/edit-controls", get(edit_album_controls_handler))
        .route(
            "/edit",
            get(edit_client_handler).post(post_edit_client_handler),
        )
        .route(
            "/delete",
            get(get_delete_album_handler).post(post_delete_album_handler),
        )
        .nest("/users", users_routes(state.clone()))
        .nest("/buckets", upload_route(state.clone()))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            client_middleware,
        ))
        .with_state(state)
}

fn users_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(users_handler))
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
            // Build the error response
            error!("{}", e.message);
            if let Some(bt) = &e.backtrace {
                error!("{}", bt);
            }
        }

        let full_page = headers.get("HX-Request").is_none();
        let actor = ctx.actor().map(|t| t.clone());
        return handle_error(&state, actor, &pref, e.clone(), full_page);
    }
    res
}
