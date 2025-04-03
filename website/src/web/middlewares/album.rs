use axum::{
    Extension,
    extract::{Path, Request, State},
    middleware::Next,
    response::Response,
};
use snafu::OptionExt;

use crate::{
    Result,
    ctx::Ctx,
    error::WhateverSnafu,
    models::AlbumParams,
    run::AppState,
    services::get_album,
    web::{Action, Resource, enforce_policy},
};

pub async fn album_listing_middleware(
    Extension(ctx): Extension<Ctx>,
    req: Request,
    next: Next,
) -> Result<Response> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Read)?;

    Ok(next.run(req).await)
}

pub async fn album_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(params): Path<AlbumParams>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Photo, Action::Read)?;

    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let album_id = params.album_id.expect("album_id is required");
    let token = ctx.token().expect("token is required");
    let album = get_album(&state.config.api_url, token, &bucket_id, &album_id).await?;

    req.extensions_mut().insert(album);
    Ok(next.run(req).await)
}
