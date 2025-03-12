use axum::{
    Extension,
    extract::{Path, Request, State},
    middleware::Next,
    response::Response,
};

use crate::{
    Error,
    ctx::Ctx,
    models::{PhotoParams, Pref},
    run::AppState,
    services::get_photo,
    web::{Action, Resource, enforce_policy, handle_error},
};

pub async fn photo_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Path(params): Path<PhotoParams>,
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

    let album_id = params.album_id.expect("album_id is required");
    let photo_id = params.photo_id.expect("photo_id is required");

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

    let config = state.config.clone();
    let result = get_photo(
        &config.api_url,
        ctx.token(),
        &bucket_id,
        &album_id,
        &photo_id,
    )
    .await;

    match result {
        Ok(photo) => {
            req.extensions_mut().insert(photo);
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
