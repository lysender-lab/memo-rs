use askama::Template;
use axum::extract::Query;
use axum::{Extension, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::pagination::PaginatedMeta;
use memo::role::Permission;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::tokens::TokenFormData;
use crate::models::{ListFilesParams, ListPhotosParams, PaginationLinks, Photo};
use crate::services::buckets::{NewBucketFormData, create_bucket, delete_bucket};
use crate::services::dirs::{Dir, NewDirFormData, SearchDirsParams, create_dir, list_dirs};
use crate::services::files::list_files;
use crate::services::photos::list_photos;
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/photo_grid.html")]
struct PhotoGridTemnplate {
    theme: String,
    bucket: BucketDto,
    dir: Dir,
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
    Extension(dir): Extension<Dir>,
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

    let config = state.config.clone();

    let auth_token = ctx.token().expect("token is required");
    let result = list_files(&config.api_url, auth_token, &cid, &bid, &dir_id, &query).await;

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
