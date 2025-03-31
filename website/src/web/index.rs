use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::{Query, State},
    response::Response,
};

use crate::{
    ctx::Ctx,
    models::{ListAlbumsParams, TemplateData},
};
use crate::{models::Pref, run::AppState};

use super::{Action, ErrorInfo, Resource, enforce_policy, handle_error};

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
) -> Response<Body> {
    let actor = ctx.actor();
    if let Err(err) = enforce_policy(actor, Resource::Album, Action::Read) {
        return handle_error(
            &state,
            Some(actor.clone()),
            &pref,
            ErrorInfo::from(&err),
            true,
        );
    }

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Home");

    let tpl = IndexTemplate {
        t,
        query_params: query.to_string(),
    };

    // Prevent caching the home page
    Response::builder()
        .status(200)
        .header("Surrogate-Control", "no-store")
        .header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate, proxy-revalidate",
        )
        .header("Pragma", "no-cache")
        .header("Expires", 0)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
