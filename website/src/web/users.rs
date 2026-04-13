use askama::Template;
use axum::debug_handler;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::client::ClientDto;
use memo::role::Permission;
use memo::user::UserDto;
use snafu::ResultExt;

use crate::models::options::SelectOption;
use crate::models::tokens::TokenFormData;
use crate::services::users::delete_user;
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::{
        token::create_csrf_token,
        users::{
            NewUserFormData, ResetPasswordFormData, UserActiveFormData, UserRoleFormData,
            create_user, list_users, reset_user_password, update_user_roles, update_user_status,
        },
    },
    web::{Action, Resource, enforce_policy},
};

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
    enforce_policy(actor, Resource::User, Action::Read)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Users");

    let token = ctx.token().expect("token is required");
    let users = list_users(&state, token, &client.id).await?;

    let tpl = UsersPageTemplate { t, client, users };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
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

    enforce_policy(actor, Resource::User, Action::Create)?;

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

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_new_user_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<NewUserFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    enforce_policy(actor, Resource::User, Action::Create)?;

    let token = create_csrf_token("new_user", &config.jwt_secret)?;
    let cid = client.id.clone();

    let mut tpl = NewUserFormTemplate {
        client,
        action: format!("/clients/{}/users/new", &cid),
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
    let result = create_user(&state, token, &cid, &user).await;

    match result {
        Ok(_) => {
            let next_url = format!("/clients/{}/users", cid.as_str());
            // Weird but can't do a redirect here, let htmx handle it
            return Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu);
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
    Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "pages/user.html")]
struct UserPageTemplate {
    t: TemplateData,
    client: ClientDto,
    user: UserDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
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

    let tpl = UserPageTemplate {
        t,
        client,
        user,
        updated: false,
        can_edit: actor.has_permissions(&vec![Permission::UsersEdit]),
        can_delete: actor.has_permissions(&vec![Permission::UsersDelete]),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
