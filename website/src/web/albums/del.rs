use askama::Template;
use axum::Form;
use axum::http::{Method, StatusCode};
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::{OptionExt, ResultExt};

use crate::error::{ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu};
use crate::{
    Result,
    ctx::Ctx,
    error::ErrorInfo,
    models::{Album, DeleteAlbumForm},
    run::AppState,
    services::{create_csrf_token, delete_album},
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/delete_album_form.html")]
struct DeleteAlbumTemplate {
    album: Album,
    payload: DeleteAlbumForm,
    error_message: Option<String>,
}

/// Deletes album then redirect or show error
pub async fn delete_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
    method: Method,
    payload: Form<DeleteAlbumForm>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let _ = enforce_policy(actor, Resource::Album, Action::Delete)?;

    let token = create_csrf_token(&album.id, &config.jwt_secret)?;

    let mut error_message: Option<String> = None;
    let mut status_code: StatusCode = StatusCode::OK;
    let auth_token = ctx.token().expect("token is required");

    if method == Method::POST {
        let result = delete_album(&config, auth_token, &bucket_id, &album.id, &payload.token).await;
        match result {
            Ok(_) => {
                // Render same form but trigger a redirect to home
                let tpl = DeleteAlbumTemplate {
                    album,
                    payload: DeleteAlbumForm {
                        token: "".to_string(),
                    },
                    error_message,
                };
                return Ok(Response::builder()
                    .status(200)
                    .header("HX-Redirect", "/")
                    .body(Body::from(tpl.render().context(TemplateSnafu)?))
                    .context(ResponseBuilderSnafu)?);
            }
            Err(err) => {
                let error_info = ErrorInfo::from(&err);
                error_message = Some(error_info.message);
                status_code = error_info.status_code;
            }
        }
    }

    // Just render the form on first load or on error
    let tpl = DeleteAlbumTemplate {
        album,
        payload: DeleteAlbumForm { token },
        error_message,
    };

    Ok(Response::builder()
        .status(status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
