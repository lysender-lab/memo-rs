use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::dir::{DirDto, DirType};
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::PaginationLinks;
use crate::models::tokens::TokenFormData;
use crate::services::dirs::{
    NewDirFormData, SearchDirsParams, UpdateDirFormData, create_dir_svc, delete_dir_svc,
    list_dirs_svc, update_dir_svc,
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
    dir_type: DirType,
    dirs: Vec<DirDto>,
    pagination: Option<PaginationLinks>,
    can_create: bool,
    error_message: Option<String>,
}

pub async fn search_dirs_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir_type): Extension<DirType>,
    State(state): State<AppState>,
    Query(query): Query<SearchDirsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Dir, Action::Read)?;

    let mut tpl = SearchDirsTemplate {
        dir_type: dir_type.clone(),
        dirs: Vec::new(),
        pagination: None,
        can_create: enforce_policy(actor, Resource::Dir, Action::Create).is_ok(),
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    match list_dirs_svc(&state, token, &dir_type, &query).await {
        Ok(dirs) => {
            let mut keyword_param: String = "".to_string();
            if let Some(keyword) = &query.keyword {
                keyword_param = format!("&keyword={}", encode(keyword));
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
    dir_type: DirType,
    payload: NewDirFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_dir_form.html")]
struct DirFormTemplate {
    dir_type: DirType,
    payload: NewDirFormData,
    error_message: Option<String>,
}

pub async fn new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir_type): Extension<DirType>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Create)?;

    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from(match dir_type {
        DirType::Videos => "Create New Album",
        _ => "Create New Directory",
    });

    let token = create_csrf_token("new_dir", &config.jwt_secret)?;

    let tpl = NewDirTemplate {
        t,
        dir_type,
        payload: NewDirFormData {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir_type): Extension<DirType>,
    State(state): State<AppState>,
    payload: Form<NewDirFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Create)?;

    let token = create_csrf_token("new_dir", &config.jwt_secret)?;

    let mut tpl = DirFormTemplate {
        dir_type: dir_type.clone(),
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
    let result = create_dir_svc(&state, token, &dir_type, dir).await;

    match result {
        Ok(_) => {
            let next_url = format!("/{}", &dir_type);
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
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    let mut t = TemplateData::new(&state, actor, &pref);

    t.title = format!("Photos - {}", &dir.label);

    let tpl = DirTemplate {
        t,
        dir,
        updated: false,
        can_edit: enforce_policy(actor, Resource::Dir, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Dir, Action::Delete).is_ok(),
        can_add_files: enforce_policy(actor, Resource::File, Action::Create).is_ok(),
        can_delete_files: enforce_policy(actor, Resource::File, Action::Delete).is_ok(),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "widgets/edit_dir_controls.html")]
struct EditDirControlsTemplate {
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
    Extension(dir): Extension<DirDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Dir, Action::Update)?;

    let tpl = EditDirControlsTemplate {
        dir,
        updated: false,
        can_edit: enforce_policy(actor, Resource::Dir, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Dir, Action::Delete).is_ok(),
        can_add_files: enforce_policy(actor, Resource::File, Action::Create).is_ok(),
        can_delete_files: enforce_policy(actor, Resource::File, Action::Delete).is_ok(),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "widgets/edit_dir_form.html")]
struct EditDirFormTemplate {
    payload: UpdateDirFormData,
    dir: DirDto,
    error_message: Option<String>,
}

/// Renders the edit album form
pub async fn edit_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Update)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let label = dir.label.clone();
    let tpl = EditDirFormTemplate {
        dir,
        payload: UpdateDirFormData { label, token },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

/// Handles the edit album submission
pub async fn post_edit_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Form<UpdateDirFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let dir_id = dir.id.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Update)?;

    let token = create_csrf_token(&dir_id, &config.jwt_secret)?;

    let mut tpl = EditDirFormTemplate {
        dir: dir.clone(),
        payload: UpdateDirFormData {
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    tpl.payload.label = payload.label.clone();

    let token = ctx.token().expect("token is required");
    let result = update_dir_svc(&state, token, &dir.dir_type, &dir_id, &payload).await;
    match result {
        Ok(updated_dir) => {
            // Render the controls again with an out-of-bound swap for title
            let tpl = EditDirControlsTemplate {
                dir: updated_dir,
                updated: true,
                can_edit: enforce_policy(actor, Resource::Dir, Action::Update).is_ok(),
                can_delete: enforce_policy(actor, Resource::Dir, Action::Delete).is_ok(),
                can_add_files: enforce_policy(actor, Resource::File, Action::Create).is_ok(),
                can_delete_files: enforce_policy(actor, Resource::File, Action::Delete).is_ok(),
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
    dir: DirDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn get_delete_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Delete)?;
    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let tpl = DeleteDirTemplate {
        dir,
        payload: TokenFormData { token },
        error_message: None,
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

/// Deletes album then redirect or show error
pub async fn post_delete_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Dir, Action::Delete)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let auth_token = ctx.token().expect("token is required");

    let result = delete_dir_svc(&state, auth_token, &dir.dir_type, &dir.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let tpl = DeleteDirTemplate {
                dir: dir.clone(),
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", format!("/{}", &dir.dir_type))
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            let error_message = Some(error_info.message);

            // Just render the form on first load or on error
            let tpl = DeleteDirTemplate {
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
