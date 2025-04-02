use askama::Template;
use axum::Form;
use axum::http::{Method, StatusCode};
use axum::{Extension, body::Body, extract::State, response::Response};

use crate::error::ErrorInfo;
use crate::models::{DeleteAlbumForm, Pref};
use crate::run::AppState;
use crate::services::{create_csrf_token, delete_album};
use crate::{ctx::Ctx, models::Album};

use crate::web::{Action, Resource, enforce_policy, handle_error};

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
    Extension(pref): Extension<Pref>,
    Extension(album): Extension<Album>,
    State(state): State<AppState>,
    method: Method,
    payload: Form<DeleteAlbumForm>,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        let error = ErrorInfo::new("No default bucket.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, false);
    };

    if let Err(err) = enforce_policy(actor, Resource::Album, Action::Delete) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            false,
        );
    }

    let Ok(token) = create_csrf_token(&album.id, &config.jwt_secret) else {
        let error = ErrorInfo::new("Failed to initialize delete album form.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, true);
    };

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
                return Response::builder()
                    .status(200)
                    .header("HX-Redirect", "/")
                    .body(Body::from(tpl.render().unwrap()))
                    .unwrap();
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

    Response::builder()
        .status(status_code)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
