use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::{Query, State},
    response::Response,
};
use snafu::ResultExt;

use crate::{
    Result,
    ctx::Ctx,
    error::TemplateSnafu,
    models::{ListAlbumsParams, TemplateData},
};
use crate::{models::Pref, run::AppState};

use super::{Action, Resource, enforce_policy};

#[derive(Template)]
#[template(path = "pages/index.html")]
struct IndexTemplate {
    t: TemplateData,
    query_params: String,
}

pub async fn index_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
    Query(query): Query<ListAlbumsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Read)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Home");

    let tpl = IndexTemplate {
        t,
        query_params: query.to_string(),
    };

    // Prevent caching the home page
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
        .unwrap())
}
