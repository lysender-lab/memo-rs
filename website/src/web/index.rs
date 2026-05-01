use askama::Template;
use axum::{Extension, body::Body, extract::State, response::Response};
use snafu::ResultExt;

use crate::{
    Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::TemplateData,
};
use crate::{models::Pref, run::AppState};

#[derive(Template)]
#[template(path = "pages/index.html")]
struct IndexTemplate {
    t: TemplateData,
}

pub async fn index_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();

    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from("Home");

    let tpl = IndexTemplate { t };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
