use askama::Template;
use axum::debug_handler;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::user::UserDto;
use snafu::ResultExt;

use crate::services::users::{ChangePasswordFormData, change_user_password};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
};

#[derive(Template)]
#[template(path = "pages/profile.html")]
struct ProfilePageTemplate {
    t: TemplateData,
    user: UserDto,
}

pub async fn profile_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("User - {}", &actor.user.username);

    let tpl = ProfilePageTemplate {
        t,
        user: actor.user.clone(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_profile_controls.html")]
struct ProfileControlsTemplate {}

pub async fn profile_controls_handler() -> Result<Response<Body>> {
    let tpl = ProfileControlsTemplate {};

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/change_user_password_form.html")]
struct ChangeUserPasswordTemplate {
    payload: ChangePasswordFormData,
    error_message: Option<String>,
}

pub async fn change_user_password_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let token = create_csrf_token(&actor.user.id, &config.jwt_secret)?;

    let tpl = ChangeUserPasswordTemplate {
        payload: ChangePasswordFormData {
            token,
            current_password: "".to_string(),
            new_password: "".to_string(),
            confirm_new_password: "".to_string(),
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[debug_handler]
pub async fn post_change_password_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
    payload: Form<ChangePasswordFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let token = create_csrf_token(&actor.user.id, &config.jwt_secret)?;

    let mut tpl = ChangeUserPasswordTemplate {
        payload: ChangePasswordFormData {
            token,
            current_password: payload.current_password.clone(),
            new_password: payload.new_password.clone(),
            confirm_new_password: payload.confirm_new_password.clone(),
        },
        error_message: None,
    };

    let data = ChangePasswordFormData {
        token: payload.token.clone(),
        current_password: payload.current_password.clone(),
        new_password: payload.new_password.clone(),
        confirm_new_password: payload.confirm_new_password.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = change_user_password(&state, token, &actor.user.id, &data).await;

    match result {
        Ok(_) => {
            let tpl = ProfileControlsTemplate {};

            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/html")
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let status;
            match err {
                Error::Validation { msg } => {
                    status = StatusCode::BAD_REQUEST;
                    tpl.error_message = Some(msg);
                }
                Error::LoginRequired => {
                    status = StatusCode::UNAUTHORIZED;
                    tpl.error_message = Some("Login required.".to_string());
                }
                any_err => {
                    status = StatusCode::INTERNAL_SERVER_ERROR;
                    tpl.error_message = Some(any_err.to_string());
                }
            };

            Ok(Response::builder()
                .status(status)
                .header("Content-Type", "text/html")
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}
