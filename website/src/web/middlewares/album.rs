use axum::{
    Extension,
    extract::{Path, Request, State},
    middleware::Next,
    response::Response,
};

use crate::{
    Error,
    ctx::Ctx,
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
    // Ensure that users has access to albums and everything under it
    let full_page = req.headers().get("HX-Request").is_none();
    if let Err(err) = enforce_policy(ctx.actor(), Resource::Album, Action::Read) {
        return handle_error(
            &state,
            Some(ctx.actor().clone()),
            &pref,
            err.into(),
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
    let full_page = req.headers().get("HX-Request").is_none();
    if let Err(err) = enforce_policy(ctx.actor(), Resource::Photo, Action::Read) {
        return handle_error(
            &state,
            Some(ctx.actor().clone()),
            &pref,
            err.into(),
            full_page,
        );
    }

    let actor = ctx.actor();
    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        return handle_error(
            &state,
            Some(ctx.actor().clone()),
            &pref,
            Error::NoDefaultBucket.into(),
            full_page,
        );
    };

    let album_id = params.album_id.expect("album_id is required");
    let result = get_album(&state.config.api_url, ctx.token(), &bucket_id, &album_id).await;

    match result {
        Ok(album) => {
            req.extensions_mut().insert(album);
        }
        Err(err) => {
            return handle_error(
                &state,
                Some(ctx.actor().clone()),
                &pref,
                err.into(),
                full_page,
            );
        }
    };

    next.run(req).await
}
