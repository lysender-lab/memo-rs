use askama::Template;
use axum::http::StatusCode;
use axum::{
    Extension, Form,
    body::Body,
    extract::{Query, State},
    response::Response,
};
use memo::client::ClientDto;
use snafu::{OptionExt, ResultExt};
use urlencoding::encode;

use crate::services::clients::create_client;
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

#[derive(Template)]
#[template(path = "pages/new_client.html")]
struct NewClientTemplate {
    t: TemplateData,
    action: String,
    payload: ClientFormSubmitData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_client_form.html")]
struct ClientFormTemplate {
    action: String,
    payload: ClientFormSubmitData,
    error_message: Option<String>,
}

pub async fn new_client_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New Client");

    let token = create_csrf_token("new_album", &config.jwt_secret)?;

    let tpl = NewClientTemplate {
        t,
        action: "/clients/new".to_string(),
        payload: ClientFormSubmitData {
            name: "".to_string(),
            status: "active".to_string(),
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_client_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
    payload: Form<ClientFormSubmitData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Create)?;

    let token = create_csrf_token("new_client", &config.jwt_secret)?;

    let mut tpl = ClientFormTemplate {
        action: "/clients/new".to_string(),
        payload: ClientFormSubmitData {
            name: "".to_string(),
            status: "active".to_string(),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let payload = ClientFormSubmitData {
        name: payload.name.clone(),
        status: payload.status.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_client(&config, token, &payload).await;

    match result {
        Ok(album) => {
            let next_url = format!("/clients/{}", &album.id);
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

    tpl.payload.name = payload.name.clone();
    tpl.payload.status = payload.status.clone();

    // Will only arrive here on error
    Ok(Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
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
