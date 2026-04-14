use askama::Template;
use axum::{body::Body, extract::State, response::Response};
use snafu::ResultExt;
use tower_cookies::{Cookie, Cookies, cookie::time::Duration};

use crate::{
    Result,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    run::AppState,
};

use super::THEME_COOKIE;

#[derive(Template)]
#[template(path = "widgets/set_theme.html")]
struct ThemeTemplate {
    t: InnerTemplate,
}

struct InnerTemplate {
    theme: String,
}

pub async fn light_theme_handler(
    cookies: Cookies,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    theme_handler(state, cookies, "light", "LightThemeSetEvent").await
}

pub async fn dark_theme_handler(
    cookies: Cookies,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    theme_handler(state, cookies, "dark", "DarkThemeSetEvent").await
}

async fn theme_handler(
    state: AppState,
    cookies: Cookies,
    theme: &str,
    event: &str,
) -> Result<Response<Body>> {
    let theme_cookie = Cookie::build((THEME_COOKIE, theme.to_string()))
        .http_only(true)
        .max_age(Duration::days(365))
        .secure(state.config.server.https)
        .path("/")
        .build();

    cookies.add(theme_cookie);

    let tpl = ThemeTemplate {
        t: InnerTemplate {
            theme: theme.to_string(),
        },
    };

    Response::builder()
        .status(200)
        .header("HX-Trigger", event)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
