use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use snafu::ResultExt;

use crate::{
    Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
};

use super::middleware::build_oauth_authorize_url;

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    t: TemplateData,
}

pub async fn login_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    if ctx.is_authenticated() {
        return Ok(Redirect::to("/").into_response());
    }

    let mut t = TemplateData::new(&state, ctx.actor(), &pref);
    t.title = String::from("Login");

    let tpl = LoginTemplate { t };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn login_handler(State(state): State<AppState>) -> impl IntoResponse {
    let authorize_url = build_oauth_authorize_url(&state);
    Redirect::to(&authorize_url)
}
