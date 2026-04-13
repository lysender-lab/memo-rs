use askama::Template;
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::ResultExt;

use crate::{
    Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
};
use yaas::user::UserDto;

#[derive(Template)]
#[template(path = "pages/profile.html")]
struct ProfilePageTemplate {
    t: TemplateData,
    user: UserDto,
}

pub async fn profile_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    let mut t = TemplateData::new(&state, actor, &pref);

    let actor = actor.clone().actor.expect("actor is required");
    t.title = format!("User - {}", &actor.user.name);

    let tpl = ProfilePageTemplate {
        t,
        user: actor.user.clone(),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "widgets/edit_profile_controls.html")]
struct ProfileControlsTemplate {}

pub async fn profile_controls_handler() -> Result<Response<Body>> {
    let tpl = ProfileControlsTemplate {};

    Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
