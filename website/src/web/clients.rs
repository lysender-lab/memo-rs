use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::{Query, State},
    response::Response,
};
use memo::client::ClientDto;
use snafu::{OptionExt, ResultExt};
use urlencoding::encode;

use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu},
    models::{Album, ListAlbumsParams, PaginationLinks, Pref, TemplateData},
    run::AppState,
    services::{clients::list_clients, photos::list_albums},
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/clients.html")]
struct ClientsTemplate {
    error_message: Option<String>,
    clients: Vec<ClientDto>,
    can_create: bool,
    can_edit: bool,
    can_delete: bool,
}

#[derive(Template)]
#[template(path = "pages/clients.html")]
struct ClientsPageTemplate {
    t: TemplateData,
}

pub async fn clients_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Read)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Clients");

    let tpl = ClientsPageTemplate { t };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn clients_listing_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let mut tpl = ClientsTemplate {
        error_message: None,
        clients: Vec::new(),
        can_create: enforce_policy(actor, Resource::Client, Action::Create).is_ok(),
        can_edit: enforce_policy(actor, Resource::Client, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Client, Action::Delete).is_ok(),
    };

    let token = ctx.token().expect("token is required");
    match list_clients(&config.api_url, token).await {
        Ok(clients) => {
            tpl.clients = clients;
            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

fn build_response(tpl: ClientsTemplate) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

fn build_error_response(mut tpl: ClientsTemplate, error: Error) -> Result<Response<Body>> {
    let error_info = ErrorInfo::from(&error);
    tpl.error_message = Some(error_info.message);

    Ok(Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
