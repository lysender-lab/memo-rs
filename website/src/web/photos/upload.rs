use askama::Template;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::{OptionExt, ResultExt};

use crate::error::ResponseBuilderSnafu;
use crate::services::photos::upload_photo;
use crate::services::token::create_csrf_token;
use crate::{
    Result,
    ctx::Ctx,
    error::{TemplateSnafu, WhateverSnafu},
    models::{Album, Photo, Pref, TemplateData, UploadParams},
    run::AppState,
    web::{Resource, enforce_policy, handle_error_message, policies::Action},
};

#[derive(Template)]
#[template(path = "pages/upload_photos.html")]
struct UploadPageTemplate {
    t: TemplateData,
    token: String,
    album: Album,
}

#[derive(Template)]
#[template(path = "widgets/photo_grid_item.html")]
struct UploadedPhotoTemplate {
    theme: String,
    photo: Photo,
}

pub async fn upload_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Photo, Action::Create)?;

    let token = create_csrf_token(&album.id, &config.jwt_secret)?;
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Photos - {} - Upload Photos", &album.label);
    t.scripts = vec![config.assets.upload_js.clone()];

    let tpl = UploadPageTemplate { t, token, album };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn upload_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
    Query(query): Query<UploadParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();

    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let token = create_csrf_token(&album.id, &config.jwt_secret)?;

    let auth_token = ctx.token().expect("token is required");
    let result = upload_photo(
        &config,
        auth_token,
        &actor.client_id,
        &bucket_id,
        &album.id,
        &headers,
        query.token,
        body,
    )
    .await;

    match result {
        Ok(photo) => {
            let tpl = UploadedPhotoTemplate {
                photo,
                theme: pref.theme,
            };
            Ok(Response::builder()
                .status(201)
                .header("X-Next-Token", token)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => Ok(handle_error_message(&err)),
    }
}
