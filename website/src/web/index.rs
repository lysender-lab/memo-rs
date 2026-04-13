use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use memo::bucket::BucketDto;
use snafu::ResultExt;

use crate::{
    Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::TemplateData,
    services::buckets::list_buckets,
};
use crate::{models::Pref, run::AppState};

use super::{Action, Resource, enforce_policy};

#[derive(Template)]
#[template(path = "pages/index.html")]
struct IndexTemplate {
    t: TemplateData,
    buckets: Vec<BucketDto>,
}

pub async fn index_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Bucket, Action::Read)?;

    if actor.is_system_admin() {
        // Redirect to clients page
        return Ok(Redirect::to("/clients").into_response());
    }

    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from("Home");

    let token = ctx.token().expect("token is required");
    let actor_dto = actor.clone().actor.expect("actor dto is required");
    let buckets = list_buckets(&state, token, &actor_dto.org_id).await?;

    let tpl = IndexTemplate { t, buckets };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
