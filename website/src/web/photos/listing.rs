use askama::Template;
use axum::extract::Query;
use axum::{Extension, body::Body, extract::State, response::Response};

use crate::models::{ListPhotosParams, PaginatedMeta, Pref};
use crate::run::AppState;
use crate::web::ErrorInfo;
use crate::{
    Error,
    ctx::Ctx,
    models::{Album, Photo, TemplateData},
    services::list_photos,
};

use crate::web::policies::{Action, Resource, enforce_policy};

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
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();
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

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

pub async fn photo_listing_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    Query(query): Query<ListPhotosParams>,
    State(state): State<AppState>,
) -> Response<Body> {
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
    let actor = ctx.actor();
    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        return build_error_response(tpl, Error::NoDefaultBucket);
    };

    let result = list_photos(&config.api_url, ctx.token(), &bucket_id, &album_id, &query).await;

    return match result {
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
    };
}

fn build_response(tpl: PhotoGridTemnplate) -> Response<Body> {
    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

fn build_error_response(mut tpl: PhotoGridTemnplate, error: Error) -> Response<Body> {
    let error_info: ErrorInfo = error.into();
    tpl.error_message = Some(error_info.message);

    Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
