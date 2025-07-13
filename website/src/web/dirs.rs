use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::dir::DirDto;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::PaginationLinks;
use crate::models::tokens::TokenFormData;
use crate::services::dirs::{
    NewDirFormData, SearchDirsParams, UpdateDirFormData, create_dir, delete_dir, list_dirs,
    update_dir,
};
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
    dirs: Vec<DirDto>,
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
    match list_dirs(&state, token, &cid, &bid, &query).await {
        Ok(dirs) => {
            let mut keyword_param: String = "".to_string();
            if let Some(keyword) = &query.keyword {
                keyword_param = format!("&keyword={}", encode(keyword).to_string());
            }
            tpl.dirs = dirs.data;
            tpl.pagination = Some(PaginationLinks::new(&dirs.meta, "", &keyword_param));

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

    let token = create_csrf_token("new_dir", &config.jwt_secret)?;
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
    let result = create_dir(&state, token, &cid, &bid, dir).await;

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
#[template(path = "pages/dir.html")]
struct DirTemplate {
    t: TemplateData,
    bucket: BucketDto,
    dir: DirDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
    can_add_files: bool,
    can_delete_files: bool,
}

pub async fn dir_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
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
        can_add_files: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
        can_delete_files: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_dir_controls.html")]
struct EditDirControlsTemplate {
    bucket: BucketDto,
    dir: DirDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
    can_add_files: bool,
    can_delete_files: bool,
}

/// Simply re-renders the edit and delete dir controls
pub async fn edit_dir_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Update)?;

    let tpl = EditDirControlsTemplate {
        bucket,
        dir,
        updated: false,
        can_edit: enforce_policy(actor, Resource::Album, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Album, Action::Delete).is_ok(),
        can_add_files: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
        can_delete_files: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_dir_form.html")]
struct EditDirFormTemplate {
    payload: UpdateDirFormData,
    bucket: BucketDto,
    dir: DirDto,
    error_message: Option<String>,
}

/// Renders the edit album form
pub async fn edit_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Update)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let label = dir.label.clone();
    let tpl = EditDirFormTemplate {
        bucket,
        dir,
        payload: UpdateDirFormData { label, token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

/// Handles the edit album submission
pub async fn post_edit_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Form<UpdateDirFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();
    let dir_id = dir.id.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Update)?;

    let token = create_csrf_token(&dir_id, &config.jwt_secret)?;

    let mut tpl = EditDirFormTemplate {
        bucket: bucket.clone(),
        dir: dir.clone(),
        payload: UpdateDirFormData {
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    tpl.payload.label = payload.label.clone();

    let token = ctx.token().expect("token is required");
    let result = update_dir(&state, token, &cid, &bid, &dir_id, &payload).await;
    match result {
        Ok(updated_dir) => {
            // Render the controls again with an out-of-bound swap for title
            let tpl = EditDirControlsTemplate {
                bucket,
                dir: updated_dir,
                updated: true,
                can_edit: enforce_policy(actor, Resource::Album, Action::Update).is_ok(),
                can_delete: enforce_policy(actor, Resource::Album, Action::Delete).is_ok(),
                can_add_files: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
                can_delete_files: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
            };
            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let status;
            match err {
                Error::Validation { msg } => {
                    status = 400;
                    tpl.error_message = Some(msg);
                }
                Error::LoginRequired => {
                    status = 401;
                    tpl.error_message = Some("Login required.".to_string());
                }
                any_err => {
                    status = 500;
                    tpl.error_message = Some(any_err.to_string());
                }
            }

            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "widgets/delete_dir_form.html")]
struct DeleteDirTemplate {
    bucket: BucketDto,
    dir: DirDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn get_delete_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Delete)?;
    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let tpl = DeleteDirTemplate {
        bucket,
        dir,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

/// Deletes album then redirect or show error
pub async fn post_delete_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Delete)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let auth_token = ctx.token().expect("token is required");

    let result = delete_dir(
        &state,
        auth_token,
        &bucket.client_id,
        &bucket.id,
        &dir.id,
        &payload.token,
    )
    .await;

    match result {
        Ok(_) => {
            let bid = bucket.id.clone();

            // Render same form but trigger a redirect to home
            let tpl = DeleteDirTemplate {
                bucket,
                dir,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", format!("/buckets/{}", &bid))
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            let error_message = Some(error_info.message);

            // Just render the form on first load or on error
            let tpl = DeleteDirTemplate {
                bucket,
                dir,
                payload: TokenFormData { token },
                error_message,
            };

            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}
