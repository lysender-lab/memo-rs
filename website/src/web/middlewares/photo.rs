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
    models::PhotoParams,
    run::AppState,
    services::get_photo,
    web::{Action, Resource, enforce_policy},
};

pub async fn photo_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Path(params): Path<PhotoParams>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Photo, Action::Read)?;

    let album_id = params.album_id.expect("album_id is required");
    let photo_id = params.photo_id.expect("photo_id is required");

    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let token = ctx.token().expect("token is required");
    let config = state.config.clone();
    let photo = get_photo(&config.api_url, token, &bucket_id, &album_id, &photo_id).await?;

    req.extensions_mut().insert(photo);
    Ok(next.run(req).await)
}
