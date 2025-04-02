use axum::{
    Extension,
    extract::{Path, Request, State},
    middleware::Next,
    response::Response,
};

use crate::{
    ctx::Ctx,
    error::ErrorInfo,
    models::{AlbumParams, Pref},
    run::AppState,
    services::get_album,
    web::{Action, Resource, enforce_policy, handle_error},
};

pub async fn album_listing_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    req: Request,
    next: Next,
) -> Response {
    let actor = ctx.actor().expect("actor is required");

    // Ensure that users has access to albums and everything under it
    let full_page = req.headers().get("HX-Request").is_none();
    if let Err(err) = enforce_policy(actor, Resource::Album, Action::Read) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            full_page,
        );
    }

    next.run(req).await
}

pub async fn album_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Path(params): Path<AlbumParams>,
    mut req: Request,
    next: Next,
) -> Response {
    let actor = ctx.actor().expect("actor is required");
    let full_page = req.headers().get("HX-Request").is_none();
    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Read) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            full_page,
        );
    }

    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        let error = ErrorInfo::new("No default bucket.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, full_page);
    };

    let album_id = params.album_id.expect("album_id is required");
    let token = ctx.token().expect("token is required");
    let result = get_album(&state.config.api_url, token, &bucket_id, &album_id).await;

    match result {
        Ok(album) => {
            req.extensions_mut().insert(album);
        }
        Err(err) => {
            return handle_error(
                &state,
                Some(actor.clone()),
                &pref,
                ErrorInfo::from(&err),
                full_page,
            );
        }
    };

    next.run(req).await
}
