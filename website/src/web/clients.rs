use askama::Template;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::client::ClientDto;
use memo::role::Permission;
use snafu::{ResultExt, ensure};

use crate::error::ForbiddenSnafu;
use crate::models::tokens::TokenFormData;
use crate::services::clients::{create_client, delete_client, update_client};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::{
        clients::{ClientFormSubmitData, list_clients},
        token::create_csrf_token,
    },
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/clients.html")]
struct ClientsTemplate {
    error_message: Option<String>,
    clients: Vec<ClientDto>,
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

    ensure!(
        actor.is_system_admin(),
        ForbiddenSnafu {
            msg: "Clients page require system admin privileges."
        }
    );

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
    let mut tpl = ClientsTemplate {
        error_message: None,
        clients: Vec::new(),
    };

    let token = ctx.token().expect("token is required");
    match list_clients(&state, token).await {
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

    let token = create_csrf_token("new_client", &config.jwt_secret)?;

    let tpl = NewClientTemplate {
        t,
        action: "/clients/new".to_string(),
        payload: ClientFormSubmitData {
            name: "".to_string(),
            active: Some("1".to_string()),
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
            active: Some("1".to_string()),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let payload = ClientFormSubmitData {
        name: payload.name.clone(),
        active: payload.active.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_client(&state, token, &payload).await;

    match result {
        Ok(_) => {
            let next_url = "/clients".to_string();
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
    tpl.payload.active = payload.active.clone();

    // Will only arrive here on error
    Ok(Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "pages/client.html")]
struct ClientPageTemplate {
    t: TemplateData,
    client: ClientDto,
    can_edit: bool,
    can_delete: bool,
    updated: bool,
}

pub async fn client_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Client - {}", &client.name);

    let tpl = ClientPageTemplate {
        t,
        client,
        can_edit: actor.has_permissions(&vec![Permission::ClientsEdit]),
        can_delete: actor.has_permissions(&vec![Permission::ClientsDelete]),
        updated: false,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_client_form.html")]
struct EditClientFormTemplate {
    client: ClientDto,
    payload: ClientFormSubmitData,
    error_message: Option<String>,
}

pub async fn edit_client_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Update)?;

    let token = create_csrf_token(&client.id, &config.jwt_secret)?;

    let tpl = EditClientFormTemplate {
        client: client.clone(),
        payload: ClientFormSubmitData {
            name: client.name,
            active: match client.status.as_str() {
                "active" => Some("1".to_string()),
                _ => None,
            },
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_edit_client_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<ClientFormSubmitData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Update)?;

    let token = create_csrf_token(&client.id, &config.jwt_secret)?;

    let mut tpl = EditClientFormTemplate {
        client: client.clone(),
        payload: ClientFormSubmitData {
            name: "".to_string(),
            active: Some("1".to_string()),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let payload = ClientFormSubmitData {
        name: payload.name.clone(),
        active: payload.active.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = update_client(&state, token, &client.id, &payload).await;

    match result {
        Ok(updated_client) => {
            // Render the controls back
            let tpl = EditClientControlsTemplate {
                client: updated_client,
                updated: true,
                can_edit: actor.has_permissions(&vec![Permission::ClientsEdit]),
                can_delete: actor.has_permissions(&vec![Permission::ClientsDelete]),
            };

            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            status = error_info.status_code;
            tpl.error_message = Some(error_info.message);

            tpl.payload.name = payload.name.clone();
            tpl.payload.active = payload.active.clone();

            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "widgets/edit_client_controls.html")]
struct EditClientControlsTemplate {
    client: ClientDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
}

pub async fn edit_client_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Update)?;

    let tpl = EditClientControlsTemplate {
        client,
        updated: false,
        can_edit: actor.has_permissions(&vec![Permission::ClientsEdit]),
        can_delete: actor.has_permissions(&vec![Permission::ClientsDelete]),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/delete_client_form.html")]
struct DeleteClientFormTemplate {
    client: ClientDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn delete_client_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Delete)?;

    let token = create_csrf_token(&client.id, &config.jwt_secret)?;

    let tpl = DeleteClientFormTemplate {
        client: client.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_delete_client_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Client, Action::Delete)?;

    let token = create_csrf_token(&client.id, &config.jwt_secret)?;

    let mut tpl = DeleteClientFormTemplate {
        client: client.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    let status: StatusCode;

    let token = ctx.token().expect("token is required");
    let result = delete_client(&state, token, &client.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let tpl = DeleteClientFormTemplate {
                client,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            return Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", "/clients")
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?);
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            status = error_info.status_code;
            tpl.error_message = Some(error_info.message);

            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
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
