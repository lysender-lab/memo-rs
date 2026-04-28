use askama::Template;
use axum::extract::Query;
use axum::{Extension, body::Body, extract::State, response::Response};
use memo::dir::DirType;
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
    dir_type: DirType,
    query_params: String,
}

pub async fn my_bucket_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir_type): Extension<DirType>,
    State(state): State<AppState>,
    Query(query): Query<ListDirsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    let mut t = TemplateData::new(&state, actor, &pref);

    t.title = dir_type.to_string().to_uppercase();

    let tpl = MyBucketPageTemplate {
        t,
        dir_type,
        query_params: query.to_string(),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}
