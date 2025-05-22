use askama::Template;
use axum::extract::Query;
use axum::{Extension, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use snafu::ResultExt;

use crate::models::ListDirsParams;
use crate::{
    Result,
    ctx::Ctx,
    error::{ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
};

#[derive(Template)]
#[template(path = "pages/my_bucket.html")]
struct MyBucketPageTemplate {
    t: TemplateData,
    bucket: BucketDto,
    query_params: String,
}

pub async fn my_bucket_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    Query(query): Query<ListDirsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Bucket - {}", &bucket.name);

    let tpl = MyBucketPageTemplate {
        t,
        bucket,
        query_params: query.to_string(),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
