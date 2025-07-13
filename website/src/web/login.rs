use std::collections::HashMap;

use askama::Template;
use axum::{
    body::Body,
    extract::{Form, Query, State},
    http::Response,
    response::{IntoResponse, Redirect},
};
use snafu::ResultExt;
use tower_cookies::{Cookie, Cookies, cookie::time::Duration};
use validator::Validate;

use crate::{
    Error, Result,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::{LoginFormPayload, TemplateData},
    services::{
        auth::{AuthPayload, authenticate},
        captcha::validate_catpcha,
    },
};
use crate::{error::ErrorInfo, models::Pref, run::AppState};
use memo::actor::Actor;

use super::AUTH_TOKEN_COOKIE;

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    t: TemplateData,
    captcha_key: String,
    error_message: Option<String>,
}

pub async fn login_handler(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response<Body>> {
    // Errors are handled via redirect with query params
    let pref = Pref::new();
    let actor: Option<Actor> = None;
    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from("Login");
    t.async_scripts = vec!["https://www.google.com/recaptcha/enterprise.js".to_string()];

    let config = state.config.clone();
    let captcha_key = config.captcha_site_key.clone();

    let mut error_message = None;
    if let Some(err) = query.get("error") {
        error_message = Some(err.to_string());
    }

    let tpl = LoginTemplate {
        t,
        captcha_key,
        error_message,
    };

    Ok(Response::builder()
        .status(200)
        .header("Surrogate-Control", "no-store")
        .header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate, proxy-revalidate",
        )
        .header("Pragma", "no-cache")
        .header("Expires", 0)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_login_handler(
    cookies: Cookies,
    State(state): State<AppState>,
    Form(login_payload): Form<LoginFormPayload>,
) -> impl IntoResponse {
    // Validate data
    if let Err(err) = login_payload.validate() {
        let errors: Vec<String> = err
            .field_errors()
            .keys()
            .map(|k| match k.as_ref() {
                "g-recaptcha-response" => "captcha".to_string(),
                other => other.to_string(),
            })
            .collect();
        let mut error_message = "Complete the form.".to_string();
        if errors.contains(&"captcha".to_string()) {
            error_message = "Click the I'm not a robot checkbox.".to_string();
        }
        return handle_error(Error::Validation { msg: error_message });
    }

    // Validate captcha
    if let Err(captcha_err) =
        validate_catpcha(&state, login_payload.g_recaptcha_response.as_str()).await
    {
        return handle_error(captcha_err);
    }

    // Validate login information
    let auth_payload = AuthPayload {
        username: login_payload.username,
        password: login_payload.password,
    };
    let login_result = authenticate(&state, auth_payload).await;
    let auth = match login_result {
        Ok(val) => val,
        Err(err) => {
            return handle_error(err);
        }
    };

    let auth_cookie = Cookie::build((AUTH_TOKEN_COOKIE, auth.token.clone()))
        .http_only(true)
        .max_age(Duration::weeks(1))
        .secure(state.config.ssl)
        .path("/")
        .build();

    cookies.add(auth_cookie);

    Redirect::to("/").into_response()
}

fn handle_error(error: Error) -> Response<Body> {
    let error_info = ErrorInfo::from(&error);

    let url = format!("/login?error={}", error_info.message);
    Redirect::to(url.as_str()).into_response()
}
