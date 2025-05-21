use askama::Template;
use axum::debug_handler;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::client::ClientDto;
use memo::role::Permission;
use memo::user::UserDto;
use snafu::ResultExt;

use crate::models::options::SelectOption;
use crate::models::tokens::TokenFormData;
use crate::services::buckets::{NewBucketFormData, create_bucket, list_buckets};
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
#[template(path = "pages/buckets.html")]
struct BucketsPageTemplate {
    t: TemplateData,
    client: ClientDto,
    buckets: Vec<BucketDto>,
}

pub async fn buckets_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Bucket, Action::Read)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Buckets");

    let token = ctx.token().expect("token is required");
    let buckets = list_buckets(state.config.api_url.as_str(), token, client.id.as_str()).await?;

    let tpl = BucketsPageTemplate { t, client, buckets };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
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

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_user_controls.html")]
struct UserControlsTemplate {
    client: ClientDto,
    user: UserDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
}

pub async fn user_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Update)?;

    let tpl = UserControlsTemplate {
        client,
        user,
        updated: false,
        can_edit: actor.has_permissions(&vec![Permission::UsersEdit]),
        can_delete: actor.has_permissions(&vec![Permission::UsersDelete]),
    };

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
    if &user.status == "active" {
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

#[debug_handler]
pub async fn post_update_user_status_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
    payload: Form<UserActiveFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Update)?;

    let token = create_csrf_token(&user.id, &config.jwt_secret)?;
    let cid = client.id.clone();
    let uid = user.id.clone();

    let mut tpl = UpdateUserStatusTemplate {
        client: client.clone(),
        user,
        payload: UserActiveFormData {
            token,
            active: payload.active.clone(),
        },
        error_message: None,
    };

    let data = UserActiveFormData {
        active: payload.active.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = update_user_status(&config, token, &cid, &uid, &data).await;

    match result {
        Ok(updated_user) => {
            // Render back the controls but when updated roles and status
            let tpl = UserControlsTemplate {
                client,
                user: updated_user,
                updated: true,
                can_edit: actor.has_permissions(&vec![Permission::UsersEdit]),
                can_delete: actor.has_permissions(&vec![Permission::UsersDelete]),
            };

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

#[derive(Template)]
#[template(path = "widgets/delete_user_form.html")]
struct DeleteUserFormTemplate {
    client: ClientDto,
    user: UserDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn delete_user_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Delete)?;

    let token = create_csrf_token(&user.id, &config.jwt_secret)?;

    let tpl = DeleteUserFormTemplate {
        client,
        user,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_delete_user_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(user): Extension<UserDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::User, Action::Delete)?;

    let token = create_csrf_token(&user.id, &config.jwt_secret)?;

    let mut tpl = DeleteUserFormTemplate {
        client: client.clone(),
        user: user.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    let result = delete_user(&config, token, &client.id, &user.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let cid = client.id.clone();
            let tpl = DeleteUserFormTemplate {
                client,
                user,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            return Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", format!("/clients/{}/users", &cid))
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
