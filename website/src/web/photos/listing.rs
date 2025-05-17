use askama::Template;
use axum::extract::Query;
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::{OptionExt, ResultExt};

use crate::error::{ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::ErrorInfo,
    models::{Album, ListPhotosParams, PaginatedMeta, Photo, Pref, TemplateData},
    run::AppState,
    services::photos::list_photos,
    web::policies::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "pages/photos.html")]
struct PhotosTemplate {
    t: TemplateData,
    album: Album,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
    can_add_photos: bool,
    can_delete_photos: bool,
}

#[derive(Template)]
#[template(path = "widgets/photo_grid.html")]
struct PhotoGridTemnplate {
    theme: String,
    album: Album,
    photos: Vec<Photo>,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    last_item: String,
}

pub async fn photos_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Photos - {}", &album.label);
    t.styles = vec![config.assets.gallery_css.clone()];
    t.scripts = vec![config.assets.gallery_js.clone()];

    let tpl = PhotosTemplate {
        t,
        album,
        updated: false,
        can_edit: enforce_policy(actor, Resource::Album, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Album, Action::Delete).is_ok(),
        can_add_photos: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
        can_delete_photos: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn photo_listing_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    Query(query): Query<ListPhotosParams>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let album_id = album.id.clone();

    let mut tpl = PhotoGridTemnplate {
        theme: pref.theme,
        album,
        photos: Vec::new(),
        meta: None,
        error_message: None,
        next_page: None,
        last_item: "".to_string(),
    };

    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket",
    })?;

    let auth_token = ctx.token().expect("token is required");
    let result = list_photos(
        &config.api_url,
        auth_token,
        &actor.client_id,
        &bucket_id,
        &album_id,
        &query,
    )
    .await;

    match result {
        Ok(listing) => {
            tpl.photos = listing.data;

            if listing.meta.total_pages > listing.meta.page {
                tpl.next_page = Some(listing.meta.page + 1);
            }

            // Get the last item
            if let Some(photo) = tpl.photos.last() {
                tpl.last_item = photo.id.clone();
            }
            tpl.meta = Some(listing.meta);

            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

fn build_response(tpl: PhotoGridTemnplate) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

fn build_error_response(mut tpl: PhotoGridTemnplate, error: Error) -> Result<Response<Body>> {
    let error_info = ErrorInfo::from(&error);
    tpl.error_message = Some(error_info.message);

    Ok(Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
