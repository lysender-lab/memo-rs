use askama::Template;
use axum::{
    Extension,
    body::Body,
    extract::{Query, State},
    response::Response,
};
use snafu::{OptionExt, ResultExt};
use urlencoding::encode;

use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu, WhateverSnafu},
    models::{Album, ListAlbumsParams, PaginationLinks},
    run::AppState,
    services::list_albums,
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/albums.html")]
struct AlbumsTemplate {
    error_message: Option<String>,
    albums: Vec<Album>,
    pagination: Option<PaginationLinks>,
    can_create: bool,
}

pub async fn album_listing_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
    Query(query): Query<ListAlbumsParams>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");
    let default_bucket_id = actor.default_bucket_id.clone();

    let mut tpl = AlbumsTemplate {
        error_message: None,
        albums: Vec::new(),
        pagination: None,
        can_create: enforce_policy(actor, Resource::Album, Action::Create).is_ok(),
    };

    let bucket_id = default_bucket_id.context(WhateverSnafu {
        msg: "No default bucket",
    })?;

    let token = ctx.token().expect("token is required");
    match list_albums(&config.api_url, token, &actor.client_id, &bucket_id, &query).await {
        Ok(albums) => {
            let mut keyword_param: String = "".to_string();
            if let Some(keyword) = &query.keyword {
                keyword_param = format!("&keyword={}", encode(keyword).to_string());
            }
            tpl.albums = albums.data;
            tpl.pagination = Some(PaginationLinks::new(&albums.meta, "", &keyword_param));
            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

fn build_response(tpl: AlbumsTemplate) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

fn build_error_response(mut tpl: AlbumsTemplate, error: Error) -> Result<Response<Body>> {
    let error_info = ErrorInfo::from(&error);
    tpl.error_message = Some(error_info.message);

    Ok(Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
