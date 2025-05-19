use askama::Template;
use axum::Form;
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::ResultExt;

use crate::error::ResponseBuilderSnafu;
use crate::models::tokens::TokenFormData;
use crate::services::photos::delete_photo;
use crate::services::token::create_csrf_token;
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, TemplateSnafu},
    models::{Album, Photo},
    run::AppState,
    web::{Action, Resource, enforce_policy, handle_error_message},
};

#[derive(Template)]
#[template(path = "widgets/pre_delete_photo_form.html")]
struct PreDeletePhotoTemplate {
    photo: Photo,
}

#[derive(Template)]
#[template(path = "widgets/confirm_delete_photo_form.html")]
struct ConfirmDeletePhotoTemplate {
    photo: Photo,
    payload: TokenFormData,
    error_message: Option<String>,
}

/// Shows pre-delete form controls
pub async fn pre_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(photo): Extension<Photo>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    // Just render the form on first load or on error
    let tpl = PreDeletePhotoTemplate { photo };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

/// Shows delete/cancel form controls
pub async fn confirm_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        let error = Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        };
        return Ok(handle_error_message(&error));
    };

    // Just render the form on first load or on error
    let tpl = ConfirmDeletePhotoTemplate {
        photo,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn exec_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        return Ok(handle_error_message(&Error::Whatever {
            msg: "No default bucket.".to_string(),
        }));
    };

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        return Ok(handle_error_message(&Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        }));
    };

    let status_code;
    let error_message;

    let auth_token = ctx.token().expect("token is required");
    let result = delete_photo(
        &config,
        auth_token,
        &actor.client_id,
        &bucket_id,
        &album.id,
        &photo.id,
        &payload.token,
    )
    .await;
    match result {
        Ok(_) => {
            return Ok(Response::builder()
                .status(204)
                .header("HX-Trigger", "PhotoDeletedEvent")
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?);
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            error_message = Some(error_info.message);
            status_code = error_info.status_code;
        }
    }

    // Re-render the form with a new token
    // We may need to render an error message somewhere in the page
    let tpl = ConfirmDeletePhotoTemplate {
        photo,
        payload: TokenFormData { token },
        error_message,
    };

    Ok(Response::builder()
        .status(status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
