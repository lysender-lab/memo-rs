use askama::Template;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use snafu::{OptionExt, ResultExt};

use crate::{
    Error, Result,
    ctx::Ctx,
    error::{TemplateSnafu, WhateverSnafu},
    models::{Album, UpdateAlbumForm},
    run::AppState,
    services::{create_csrf_token, update_album},
};

use crate::web::{Action, Resource, enforce_policy};

#[derive(Template)]
#[template(path = "widgets/edit_album_form.html")]
struct EditAlbumFormTemplate {
    payload: UpdateAlbumForm,
    album: Album,
    error_message: Option<String>,
    updated: bool,
}

#[derive(Template)]
#[template(path = "widgets/edit_album_controls.html")]
struct EditAlbumControlsTemplate {
    album: Album,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
    can_add_photos: bool,
    can_delete_photos: bool,
}

/// Simply re-renders the edit and delete album controls
pub async fn edit_album_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let tpl = EditAlbumControlsTemplate {
        album,
        updated: false,
        can_edit: enforce_policy(actor, Resource::Album, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::Album, Action::Delete).is_ok(),
        can_add_photos: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
        can_delete_photos: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .unwrap())
}

/// Renders the edit album form
pub async fn edit_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Update)?;

    let token = create_csrf_token(&album.id, &config.jwt_secret)?;

    let label = album.label.clone();
    let tpl = EditAlbumFormTemplate {
        album,
        payload: UpdateAlbumForm { label, token },
        error_message: None,
        updated: false,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .unwrap())
}

/// Handles the edit album submission
pub async fn post_edit_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
    payload: Form<UpdateAlbumForm>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let album_id = album.id.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();
    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket.",
    })?;

    let _ = enforce_policy(actor, Resource::Album, Action::Update)?;

    let token = create_csrf_token(&album.id, &config.jwt_secret)?;

    let mut tpl = EditAlbumFormTemplate {
        album,
        payload: UpdateAlbumForm {
            label: "".to_string(),
            token,
        },
        error_message: None,
        updated: false,
    };

    let mut status = 200;
    tpl.payload.label = payload.label.clone();

    let token = ctx.token().expect("token is required");
    let result = update_album(&config, token, &bucket_id, &album_id, &payload).await;
    match result {
        Ok(updated_album) => {
            tpl.album = updated_album;
            tpl.updated = true;
        }
        Err(err) => match err {
            Error::Validation { msg } => {
                status = 400;
                tpl.error_message = Some(msg);
            }
            Error::LoginRequired => {
                status = 401;
                tpl.error_message = Some("Login required.".to_string());
            }
            any_err => {
                status = 500;
                tpl.error_message = Some(any_err.to_string());
            }
        },
    }

    if tpl.updated {
        // Render the controls again with an out-of-bound swap for title
        let tpl = EditAlbumControlsTemplate {
            album: tpl.album,
            updated: true,
            can_edit: enforce_policy(actor, Resource::Album, Action::Update).is_ok(),
            can_delete: enforce_policy(actor, Resource::Album, Action::Delete).is_ok(),
            can_add_photos: enforce_policy(actor, Resource::Photo, Action::Create).is_ok(),
            can_delete_photos: enforce_policy(actor, Resource::Photo, Action::Delete).is_ok(),
        };
        Ok(Response::builder()
            .status(status)
            .body(Body::from(tpl.render().context(TemplateSnafu)?))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(status)
            .body(Body::from(tpl.render().context(TemplateSnafu)?))
            .unwrap())
    }
}
