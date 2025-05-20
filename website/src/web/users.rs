use askama::Template;
use axum::http::StatusCode;
use axum::{
    Extension, Form,
    body::Body,
    extract::{Query, State},
    response::Response,
};
use memo::client::ClientDto;
use memo::user::UserDto;
use snafu::{OptionExt, ResultExt};
use urlencoding::encode;

use crate::models::options::SelectOption;
use crate::models::users::{
    NewUserFormData, ResetPasswordFormData, UserActiveFormData, UserRoleFormData,
};
use crate::services::clients::{create_client, update_client};
use crate::services::photos::create_album;
use crate::services::users::{create_user, list_users};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu},
    models::{
        Album, ListAlbumsParams, PaginationLinks, Pref, TemplateData, clients::ClientFormSubmitData,
    },
    run::AppState,
    services::{clients::list_clients, photos::list_albums, token::create_csrf_token},
    web::{Action, Resource, enforce_policy},
};

// #[derive(Template)]
// #[template(path = "widgets/users.html")]
// struct UsersTemplate {
//     error_message: Option<String>,
//     client: ClientDto,
//     users: Vec<UserDto>,
//     can_create: bool,
//     can_edit: bool,
//     can_delete: bool,
// }

#[derive(Template)]
#[template(path = "pages/users.html")]
struct UsersPageTemplate {
    t: TemplateData,
    client: ClientDto,
    users: Vec<UserDto>,
}

pub async fn users_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::User, Action::Read)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Users");

    let token = ctx.token().expect("token is required");
    let users = list_users(state.config.api_url.as_str(), token, client.id.as_str()).await?;

    let tpl = UsersPageTemplate { t, client, users };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "pages/new_user.html")]
struct NewUserTemplate {
    t: TemplateData,
    client: ClientDto,
    action: String,
    payload: NewUserFormData,
    role_options: Vec<SelectOption>,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_user_form.html")]
struct NewUserFormTemplate {
    client: ClientDto,
    action: String,
    payload: NewUserFormData,
    role_options: Vec<SelectOption>,
    error_message: Option<String>,
}

fn create_role_options() -> Vec<SelectOption> {
    vec![
        SelectOption {
            value: "Admin".to_string(),
            label: "Admin".to_string(),
        },
        SelectOption {
            value: "Editor".to_string(),
            label: "Editor".to_string(),
        },
        SelectOption {
            value: "Viewer".to_string(),
            label: "Viewer".to_string(),
        },
    ]
}

pub async fn new_user_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New User");

    let token = create_csrf_token("new_user", &config.jwt_secret)?;
    let cid = client.id.clone();

    let tpl = NewUserTemplate {
        t,
        client,
        action: format!("/clients/{}/users/new", cid),
        payload: NewUserFormData {
            username: "".to_string(),
            password: "".to_string(),
            confirm_password: "".to_string(),
            role: "".to_string(),
            token,
        },
        role_options: create_role_options(),
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_user_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<NewUserFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Create)?;

    let token = create_csrf_token("new_user", &config.jwt_secret)?;
    let cid = client.id.clone();

    let mut tpl = NewUserFormTemplate {
        client,
        action: format!("/clients/{}/users/new", cid.as_str()),
        payload: NewUserFormData {
            username: "".to_string(),
            password: "".to_string(),
            confirm_password: "".to_string(),
            role: "".to_string(),
            token,
        },
        role_options: create_role_options(),
        error_message: None,
    };

    let status: StatusCode;

    let user = NewUserFormData {
        username: payload.username.clone(),
        password: payload.password.clone(),
        confirm_password: payload.confirm_password.clone(),
        role: payload.role.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_user(&config, token, cid.as_str(), &user).await;

    match result {
        Ok(_) => {
            let next_url = format!("/clients/{}/users", cid.as_str());
            // Weird but can't do a redirect here, let htmx handle it
            return Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?);
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            status = error_info.status_code;
            tpl.error_message = Some(error_info.message);
        }
    }

    tpl.payload.username = payload.username.clone();
    tpl.payload.role = payload.role.clone();

    // Will only arrive here on error
    Ok(Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "pages/user.html")]
struct UserPageTemplate {
    t: TemplateData,
    client: ClientDto,
    user: UserDto,
}

pub async fn user_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("User - {}", &user.username);

    let tpl = UserPageTemplate { t, client, user };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/user_controls.html")]
struct UserControlsTemplate {
    client: ClientDto,
    user: UserDto,
}

pub async fn user_controls_handler(
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
) -> Result<Response<Body>> {
    let tpl = UserControlsTemplate { client, user };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/update_user_status_form.html")]
struct UpdateUserStatusTemplate {
    client: ClientDto,
    user: UserDto,
    payload: UserActiveFormData,
    error_message: Option<String>,
}

pub async fn update_user_status_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Update)?;
    let token = create_csrf_token(&user.id, &config.jwt_secret)?;

    let mut status_opt = None;
    if &user.status == "status" {
        status_opt = Some("1".to_string());
    }

    let tpl = UpdateUserStatusTemplate {
        client,
        user,
        payload: UserActiveFormData {
            token,
            active: status_opt,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/update_user_role_form.html")]
struct UpdateUserRoleTemplate {
    client: ClientDto,
    user: UserDto,
    payload: UserRoleFormData,
    role_options: Vec<SelectOption>,
    error_message: Option<String>,
}

pub async fn update_user_role_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Update)?;
    let token = create_csrf_token(&user.id, &config.jwt_secret)?;

    let current_role = user.roles.first().unwrap().to_string();

    let tpl = UpdateUserRoleTemplate {
        client,
        user,
        payload: UserRoleFormData {
            token,
            role: current_role,
        },
        role_options: create_role_options(),
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/reset_user_password_form.html")]
struct ResetUserPasswordTemplate {
    client: ClientDto,
    user: UserDto,
    payload: ResetPasswordFormData,
    error_message: Option<String>,
}

pub async fn reset_user_password_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Update)?;
    let token = create_csrf_token(&user.id, &config.jwt_secret)?;

    let tpl = ResetUserPasswordTemplate {
        client,
        user,
        payload: ResetPasswordFormData {
            token,
            password: "".to_string(),
            confirm_password: "".to_string(),
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
