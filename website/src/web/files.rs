use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::client::ClientDto;
use memo::file::FileDto;
use memo::pagination::PaginatedMeta;
use memo::role::Permission;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::tokens::TokenFormData;
use crate::models::{ListPhotosParams, PaginationLinks};
use crate::services::buckets::{NewBucketFormData, create_bucket, delete_bucket};
use crate::services::dirs::{Dir, NewDirFormData, SearchDirsParams, create_dir, list_dirs};
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
#[template(path = "pages/photos.html")]
struct DirTemplate {
    t: TemplateData,
    bucket: BucketDto,
    dir: Dir,
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
    bucket: BucketDto,
    dir: Dir,
    files: Vec<FileDto>,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    last_item: String,
}

pub async fn dir_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Photos - {}", &dir.label);
    t.styles = vec![config.assets.gallery_css.clone()];
    t.scripts = vec![config.assets.gallery_js.clone()];

    let tpl = DirTemplate {
        t,
        bucket,
        dir,
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

pub async fn file_listing_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    Query(query): Query<ListPhotosParams>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let dir_id = dir.id.clone();

    let mut tpl = PhotoGridTemnplate {
        theme: pref.theme,
        bucket,
        dir,
        files: Vec::new(),
        meta: None,
        error_message: None,
        next_page: None,
        last_item: "".to_string(),
    };

    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let auth_token = ctx.token().expect("token is required");
    let result = list_photos(
        &config.api_url,
        auth_token,
        &actor.client_id,
        &bucket_id,
        &dir_id,
        &query,
    )
    .await;

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

            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

#[derive(Template)]
#[template(path = "pages/new_bucket.html")]
struct NewBucketTemplate {
    t: TemplateData,
    client: ClientDto,
    payload: NewBucketFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_bucket_form.html")]
struct NewBucketFormTemplate {
    client: ClientDto,
    payload: NewBucketFormData,
    error_message: Option<String>,
}

pub async fn new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New Bucket");

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;

    let tpl = NewBucketTemplate {
        t,
        client,
        payload: NewBucketFormData {
            name: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<NewBucketFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;
    let cid = client.id.clone();

    let mut tpl = NewBucketFormTemplate {
        client,
        payload: NewBucketFormData {
            name: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    let bucket = NewBucketFormData {
        name: payload.name.clone(),
        images_only: payload.images_only.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_bucket(&config, token, cid.as_str(), &bucket).await;

    match result {
        Ok(_) => {
            let next_url = format!("/clients/{}/buckets", cid.as_str());
            // Weird but can't do a redirect here, let htmx handle it
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            tpl.error_message = Some(error_info.message);

            tpl.payload.name = payload.name.clone();
            tpl.payload.images_only = payload.images_only.clone();

            // Will only arrive here on error
            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "pages/bucket.html")]
struct BucketPageTemplate {
    t: TemplateData,
    client: ClientDto,
    bucket: BucketDto,
    can_delete: bool,
}

pub async fn bucket_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Bucket - {}", &bucket.name);

    let tpl = BucketPageTemplate {
        t,
        client,
        bucket,
        can_delete: actor.has_permissions(&vec![Permission::BucketsDelete]),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_bucket_controls.html")]
struct BucketControlsTemplate {
    client: ClientDto,
    bucket: BucketDto,
    can_delete: bool,
}

pub async fn bucket_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Update)?;

    let tpl = BucketControlsTemplate {
        client,
        bucket,
        can_delete: actor.has_permissions(&vec![Permission::BucketsDelete]),
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/delete_bucket_form.html")]
struct DeleteBucketFormTemplate {
    client: ClientDto,
    bucket: BucketDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let tpl = DeleteBucketFormTemplate {
        client,
        bucket,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let mut tpl = DeleteBucketFormTemplate {
        client: client.clone(),
        bucket: bucket.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    let result = delete_bucket(&config, token, &client.id, &bucket.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let cid = client.id.clone();
            let tpl = DeleteBucketFormTemplate {
                client,
                bucket,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            return Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", format!("/clients/{}/buckets", &cid))
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?);
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

fn build_response(tpl: SearchDirsTemplate) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

fn build_error_response(mut tpl: SearchDirsTemplate, error: Error) -> Result<Response<Body>> {
    let error_info = ErrorInfo::from(&error);
    tpl.error_message = Some(error_info.message);

    Ok(Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
