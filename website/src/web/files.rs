use askama::Template;
use axum::Form;
use axum::body::Bytes;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::{Extension, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::dir::DirDto;
use memo::pagination::PaginatedMeta;
use snafu::ResultExt;

use crate::models::tokens::TokenFormData;
use crate::models::{ListFilesParams, UploadParams};
use crate::services::files::{Photo, delete_photo, list_files, upload_photo};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
    web::{Action, Resource, enforce_policy},
};

use super::handle_error_message;

#[derive(Template)]
#[template(path = "widgets/photo_grid.html")]
struct PhotoGridTemnplate {
    theme: String,
    bucket: BucketDto,
    dir: DirDto,
    photos: Vec<Photo>,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    last_item: String,
}

pub async fn photo_listing_v2_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    Query(query): Query<ListFilesParams>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Photo, Action::Read)?;

    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();
    let dir_id = dir.id.clone();

    let mut tpl = PhotoGridTemnplate {
        theme: pref.theme,
        bucket,
        dir,
        photos: Vec::new(),
        meta: None,
        error_message: None,
        next_page: None,
        last_item: "".to_string(),
    };

    let auth_token = ctx.token().expect("token is required");
    let result = list_files(&state, auth_token, &cid, &bid, &dir_id, &query).await;

    match result {
        Ok(listing) => {
            tpl.photos = listing.data;

            if listing.meta.total_pages > listing.meta.page as i64 {
                tpl.next_page = Some(listing.meta.page as i64 + 1);
            }

            // Get the last item
            if let Some(photo) = tpl.photos.last() {
                tpl.last_item = photo.id.clone();
            }
            tpl.meta = Some(listing.meta);

            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            tpl.error_message = Some(error_info.message);

            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "pages/upload_photos.html")]
struct UploadPageTemplate {
    t: TemplateData,
    bucket: BucketDto,
    dir: DirDto,
    token: String,
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
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();

    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Photo, Action::Create)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Photos - {} - Upload Photos", &dir.label);
    t.scripts = vec![config.assets.upload_js.clone()];

    let tpl = UploadPageTemplate {
        t,
        bucket,
        dir,
        token,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn upload_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    Query(query): Query<UploadParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Photo, Action::Create)?;

    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let auth_token = ctx.token().expect("token is required");
    let result = upload_photo(
        &state,
        auth_token,
        &cid,
        &bid,
        &dir.id,
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

#[derive(Template)]
#[template(path = "widgets/pre_delete_photo_form.html")]
struct PreDeletePhotoTemplate {
    bucket: BucketDto,
    dir: DirDto,
    photo: Photo,
}

#[derive(Template)]
#[template(path = "widgets/confirm_delete_photo_form.html")]
struct ConfirmDeletePhotoTemplate {
    bucket: BucketDto,
    dir: DirDto,
    photo: Photo,
    payload: TokenFormData,
    error_message: Option<String>,
}

/// Shows pre-delete form controls
pub async fn pre_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    // Just render the form on first load or on error
    let tpl = PreDeletePhotoTemplate { bucket, dir, photo };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

/// Shows delete/cancel form controls
pub async fn confirm_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        let error = Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        };
        return Ok(handle_error_message(&error));
    };

    // Just render the form on first load or on error
    let tpl = ConfirmDeletePhotoTemplate {
        bucket,
        dir,
        photo,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn exec_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();
    let dir_id = dir.id.clone();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        return Ok(handle_error_message(&Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        }));
    };

    let auth_token = ctx.token().expect("token is required");
    let result = delete_photo(
        &state,
        auth_token,
        &cid,
        &bid,
        &dir_id,
        &photo.id,
        &payload.token,
    )
    .await;
    match result {
        Ok(_) => {
            return Ok(Response::builder()
                .status(204)
                .header("HX-Trigger", "PhotoDeletedEvent")
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?);
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);

            // Re-render the form with a new token
            // We may need to render an error message somewhere in the page
            let tpl = ConfirmDeletePhotoTemplate {
                bucket,
                dir,
                photo,
                payload: TokenFormData { token },
                error_message: Some(error_info.message),
            };

            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}
