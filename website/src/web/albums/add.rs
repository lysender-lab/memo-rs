use askama::Template;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};

use crate::models::{NewAlbumForm, Pref};
use crate::run::AppState;
use crate::services::create_csrf_token;
use crate::{ctx::Ctx, models::TemplateData, services::create_album};

use crate::web::{Action, ErrorInfo, Resource, enforce_policy, handle_error};

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

#[axum::debug_handler]
pub async fn new_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Album, Action::Create) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            true,
        );
    }

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New Album");

    let Ok(token) = create_csrf_token("new_album", &config.jwt_secret) else {
        let error = ErrorInfo::new("Failed to initialize new album form.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, true);
    };

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

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}

#[axum::debug_handler]
pub async fn post_new_album_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
    payload: Form<NewAlbumForm>,
) -> Response<Body> {
    let config = state.config.clone();
    let actor = ctx.actor();
    let default_bucket_id = actor.default_bucket_id.clone();
    let Some(bucket_id) = default_bucket_id else {
        let error = ErrorInfo::new("No default bucket.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, false);
    };

    if let Err(err) = enforce_policy(actor, Resource::Album, Action::Create) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            false,
        );
    }

    let Ok(token) = create_csrf_token("new_album", &config.jwt_secret) else {
        let error = ErrorInfo::new("Failed to initialize new album form.".to_string());
        return handle_error(&state, Some(actor.clone()), &pref, error, true);
    };

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

    let result = create_album(&config, ctx.token(), &bucket_id, album).await;

    match result {
        Ok(album) => {
            let next_url = format!("/albums/{}", &album.id);
            // Weird but can't do a redirect here, let htmx handle it
            return Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .unwrap();
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
    Response::builder()
        .status(status)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
