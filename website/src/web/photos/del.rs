use askama::Template;
use axum::Form;
use axum::{Extension, body::Body, extract::State, response::Response};

use crate::models::{DeletePhotoForm, Photo};
use crate::run::AppState;
use crate::services::{create_csrf_token, delete_photo};
use crate::web::{Action, Resource, enforce_policy, handle_error_message};
use crate::{Error, ctx::Ctx, error::ErrorInfo, models::Album};

#[derive(Template)]
#[template(path = "widgets/pre_delete_photo_form.html")]
struct PreDeletePhotoTemplate {
    photo: Photo,
}

#[derive(Template)]
#[template(path = "widgets/confirm_delete_photo_form.html")]
struct ConfirmDeletePhotoTemplate {
    photo: Photo,
    payload: DeletePhotoForm,
    error_message: Option<String>,
}

/// Shows pre-delete form controls
pub async fn pre_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(photo): Extension<Photo>,
) -> Response<Body> {
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return handle_error_message(&err);
    }

    // Just render the form on first load or on error
    let tpl = PreDeletePhotoTemplate { photo };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

/// Shows delete/cancel form controls
pub async fn confirm_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return handle_error_message(&err);
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        let error = Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        };
        return handle_error_message(&error);
    };

    // Just render the form on first load or on error
    let tpl = ConfirmDeletePhotoTemplate {
        photo,
        payload: DeletePhotoForm { token },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

pub async fn exec_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(album): Extension<Album>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
    payload: Form<DeletePhotoForm>,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();
    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        return handle_error_message(&Error::Whatever {
            msg: "No default bucket.".to_string(),
        });
    };

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return handle_error_message(&err);
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        return handle_error_message(&Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        });
    };

    let status_code;
    let error_message;

    let result = delete_photo(
        &config,
        ctx.token(),
        &bucket_id,
        &album.id,
        &photo.id,
        &payload.token,
    )
    .await;
    match result {
        Ok(_) => {
            return Response::builder()
                .status(204)
                .header("HX-Trigger", "PhotoDeletedEvent")
                .body(Body::from("".to_string()))
                .unwrap();
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
        payload: DeletePhotoForm { token },
        error_message,
    };

    Response::builder()
        .status(status_code)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
