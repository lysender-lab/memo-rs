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

use crate::services::clients::{create_client, update_client};
use crate::services::users::list_users;
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
