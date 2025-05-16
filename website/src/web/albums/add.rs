use askama::Template;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use snafu::{OptionExt, ResultExt};

use crate::error::{ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu};
use crate::{
    Result,
    ctx::Ctx,
    error::ErrorInfo,
    models::{NewAlbumForm, Pref, TemplateData},
    run::AppState,
    services::{create_album, create_csrf_token},
};

use crate::web::{Action, Resource, enforce_policy};

#[derive(Template)]
#[template(path = "pages/new_album.html")]
struct NewAlbumTemplate {
    t: TemplateData,
    action: String,
    payload: NewAlbumForm,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_album_form.html")]
struct AlbumFormTemplate {
    action: String,
    payload: NewAlbumForm,
    error_message: Option<String>,
}

pub async fn new_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New Album");

    let token = create_csrf_token("new_album", &config.jwt_secret)?;

    let tpl = NewAlbumTemplate {
        t,
        action: "/albums/new".to_string(),
        payload: NewAlbumForm {
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

pub async fn post_new_album_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
    payload: Form<NewAlbumForm>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let token = create_csrf_token("new_album", &config.jwt_secret)?;

    let mut tpl = AlbumFormTemplate {
        action: "/albums/new".to_string(),
        payload: NewAlbumForm {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let album = NewAlbumForm {
        name: payload.name.clone(),
        label: payload.label.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_album(&config, token, &actor.client_id, &bucket_id, album).await;

    match result {
        Ok(album) => {
            let next_url = format!("/albums/{}", &album.id);
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
    tpl.payload.label = payload.label.clone();

    // Will only arrive here on error
    Ok(Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
