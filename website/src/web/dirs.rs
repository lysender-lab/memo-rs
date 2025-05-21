use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::PaginationLinks;
use crate::models::tokens::TokenFormData;
use crate::services::dirs::{Dir, NewDirFormData, SearchDirsParams, create_dir, list_dirs};
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
#[template(path = "widgets/search_dirs.html")]
struct SearchDirsTemplate {
    bucket: BucketDto,
    dirs: Vec<Dir>,
    pagination: Option<PaginationLinks>,
    can_create: bool,
    error_message: Option<String>,
}

pub async fn search_dirs_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    Query(query): Query<SearchDirsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Read)?;

    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();

    let mut tpl = SearchDirsTemplate {
        bucket,
        dirs: Vec::new(),
        pagination: None,
        can_create: enforce_policy(actor, Resource::Album, Action::Create).is_ok(),
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    match list_dirs(&state.config.api_url, token, &cid, &bid, &query).await {
        Ok(dirs) => {
            let mut keyword_param: String = "".to_string();
            if let Some(keyword) = &query.keyword {
                keyword_param = format!("&keyword={}", encode(keyword).to_string());
            }
            tpl.dirs = dirs.data;
            tpl.pagination = Some(PaginationLinks::new(&dirs.meta, "", &keyword_param));
            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

#[derive(Template)]
#[template(path = "pages/new_dir.html")]
struct NewDirTemplate {
    t: TemplateData,
    bucket: BucketDto,
    payload: NewDirFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_dir_form.html")]
struct DirFormTemplate {
    bucket: BucketDto,
    payload: NewDirFormData,
    error_message: Option<String>,
}

pub async fn new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from(match &bucket.images_only {
        &true => "Create New Album",
        &false => "Create New Directory",
    });

    let token = create_csrf_token("new_dir", &config.jwt_secret)?;

    let tpl = NewDirTemplate {
        t,
        bucket,
        payload: NewDirFormData {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<NewDirFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let token = create_csrf_token("new_album", &config.jwt_secret)?;
    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();

    let mut tpl = DirFormTemplate {
        bucket,
        payload: NewDirFormData {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let dir = NewDirFormData {
        name: payload.name.clone(),
        label: payload.label.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_dir(&config, token, &cid, &bid, dir).await;

    match result {
        Ok(_) => {
            let next_url = format!("/buckets/{}", &bid);
            // Weird but can't do a redirect here, let htmx handle it
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            status = error_info.status_code;
            tpl.error_message = Some(error_info.message);

            tpl.payload.name = payload.name.clone();
            tpl.payload.label = payload.label.clone();

            // Will only arrive here on error
            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

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
