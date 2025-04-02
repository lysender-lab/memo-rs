use askama::Template;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::{Extension, body::Body, extract::State, response::Response};

use crate::models::{Pref, UploadParams};
use crate::run::AppState;
use crate::services::{create_csrf_token, upload_photo};
use crate::web::{handle_error, handle_error_message};
use crate::{
    ctx::Ctx,
    error::ErrorInfo,
    models::{Album, Photo, TemplateData},
};

use crate::web::policies::{Action, Resource, enforce_policy};

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
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Create) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            true,
        );
    }
    let Ok(token) = create_csrf_token(&album.id, &config.jwt_secret) else {
        let error = ErrorInfo::new("Failed to initialize upload photos form.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, true);
    };
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Photos - {} - Upload Photos", &album.label);
    t.scripts = vec![config.assets.upload_js.clone()];

    let tpl = UploadPageTemplate { t, token, album };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

pub async fn upload_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
    Query(query): Query<UploadParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();
    let default_bucket_id = actor.default_bucket_id.clone();

    let Some(bucket_id) = default_bucket_id else {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::new("No default bucket.".to_string()),
            false,
        );
    };

    let Ok(token) = create_csrf_token(&album.id, &config.jwt_secret) else {
        let error = ErrorInfo::new("Failed to initialize upload photos form.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, true);
    };

    let result = upload_photo(
        &config,
        ctx.token(),
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
            Response::builder()
                .status(201)
                .header("X-Next-Token", token)
                .body(Body::from(tpl.render().unwrap()))
                .unwrap()
        }
        Err(err) => handle_error_message(&err),
    }
}
