use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use memo::client::ClientDto;
use memo::role::Permission;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::PaginationLinks;
use crate::models::tokens::TokenFormData;
use crate::services::buckets::{NewBucketFormData, create_bucket, delete_bucket};
use crate::services::dirs::{Dir, NewDirFormData, SearchDirsParams, create_dir, list_dirs};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "widgets/search_dirs.html")]
struct SearchDirsTemplate {
    bucket: BucketDto,
    dirs: Vec<Dir>,
    pagination: Option<PaginationLinks>,
    can_create: bool,
    error_message: Option<String>,
}

pub async fn search_dirs_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    Query(query): Query<SearchDirsParams>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let _ = enforce_policy(actor, Resource::Album, Action::Read)?;

    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();

    let mut tpl = SearchDirsTemplate {
        bucket,
        dirs: Vec::new(),
        pagination: None,
        can_create: enforce_policy(actor, Resource::Album, Action::Create).is_ok(),
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    match list_dirs(&state.config.api_url, token, &cid, &bid, &query).await {
        Ok(dirs) => {
            let mut keyword_param: String = "".to_string();
            if let Some(keyword) = &query.keyword {
                keyword_param = format!("&keyword={}", encode(keyword).to_string());
            }
            tpl.dirs = dirs.data;
            tpl.pagination = Some(PaginationLinks::new(&dirs.meta, "", &keyword_param));
            build_response(tpl)
        }
        Err(err) => build_error_response(tpl, err),
    }
}

#[derive(Template)]
#[template(path = "pages/new_dir.html")]
struct NewDirTemplate {
    t: TemplateData,
    bucket: BucketDto,
    payload: NewDirFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_dir_form.html")]
struct DirFormTemplate {
    bucket: BucketDto,
    payload: NewDirFormData,
    error_message: Option<String>,
}

pub async fn new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from(match &bucket.images_only {
        &true => "Create New Album",
        &false => "Create New Directory",
    });

    let token = create_csrf_token("new_dir", &config.jwt_secret)?;

    let tpl = NewDirTemplate {
        t,
        bucket,
        payload: NewDirFormData {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_dir_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<NewDirFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Album, Action::Create)?;

    let token = create_csrf_token("new_album", &config.jwt_secret)?;
    let cid = bucket.client_id.clone();
    let bid = bucket.id.clone();

    let mut tpl = DirFormTemplate {
        bucket,
        payload: NewDirFormData {
            name: "".to_string(),
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    let status: StatusCode;

    let dir = NewDirFormData {
        name: payload.name.clone(),
        label: payload.label.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_dir(&config, token, &cid, &bid, dir).await;

    match result {
        Ok(_) => {
            let next_url = format!("/buckets/{}", &bid);
            // Weird but can't do a redirect here, let htmx handle it
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            status = error_info.status_code;
            tpl.error_message = Some(error_info.message);

            tpl.payload.name = payload.name.clone();
            tpl.payload.label = payload.label.clone();

            // Will only arrive here on error
            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "pages/new_bucket.html")]
struct NewBucketTemplate {
    t: TemplateData,
    client: ClientDto,
    payload: NewBucketFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_bucket_form.html")]
struct NewBucketFormTemplate {
    client: ClientDto,
    payload: NewBucketFormData,
    error_message: Option<String>,
}

pub async fn new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);
    t.title = String::from("Create New Bucket");

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;

    let tpl = NewBucketTemplate {
        t,
        client,
        payload: NewBucketFormData {
            name: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    State(state): State<AppState>,
    payload: Form<NewBucketFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;
    let cid = client.id.clone();

    let mut tpl = NewBucketFormTemplate {
        client,
        payload: NewBucketFormData {
            name: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    let bucket = NewBucketFormData {
        name: payload.name.clone(),
        images_only: payload.images_only.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_bucket(&config, token, cid.as_str(), &bucket).await;

    match result {
        Ok(_) => {
            let next_url = format!("/clients/{}/buckets", cid.as_str());
            // Weird but can't do a redirect here, let htmx handle it
            Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", next_url)
                .body(Body::from("".to_string()))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            tpl.error_message = Some(error_info.message);

            tpl.payload.name = payload.name.clone();
            tpl.payload.images_only = payload.images_only.clone();

            // Will only arrive here on error
            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "pages/bucket.html")]
struct BucketPageTemplate {
    t: TemplateData,
    client: ClientDto,
    bucket: BucketDto,
    can_delete: bool,
}

pub async fn bucket_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");
    let mut t = TemplateData::new(&state, Some(actor.clone()), &pref);

    t.title = format!("Bucket - {}", &bucket.name);

    let tpl = BucketPageTemplate {
        t,
        client,
        bucket,
        can_delete: actor.has_permissions(&vec![Permission::BucketsDelete]),
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/edit_bucket_controls.html")]
struct BucketControlsTemplate {
    client: ClientDto,
    bucket: BucketDto,
    can_delete: bool,
}

pub async fn bucket_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Update)?;

    let tpl = BucketControlsTemplate {
        client,
        bucket,
        can_delete: actor.has_permissions(&vec![Permission::BucketsDelete]),
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

#[derive(Template)]
#[template(path = "widgets/delete_bucket_form.html")]
struct DeleteBucketFormTemplate {
    client: ClientDto,
    bucket: BucketDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let tpl = DeleteBucketFormTemplate {
        client,
        bucket,
        payload: TokenFormData { token },
        error_message: None,
    };

    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

pub async fn post_delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(client): Extension<ClientDto>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor().expect("actor is required");

    let _ = enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let mut tpl = DeleteBucketFormTemplate {
        client: client.clone(),
        bucket: bucket.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    let result = delete_bucket(&config, token, &client.id, &bucket.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let cid = client.id.clone();
            let tpl = DeleteBucketFormTemplate {
                client,
                bucket,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            return Ok(Response::builder()
                .status(200)
                .header("HX-Redirect", format!("/clients/{}/buckets", &cid))
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?);
        }
        Err(err) => {
            let error_info = ErrorInfo::from(&err);
            tpl.error_message = Some(error_info.message);

            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

fn build_response(tpl: SearchDirsTemplate) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}

fn build_error_response(mut tpl: SearchDirsTemplate, error: Error) -> Result<Response<Body>> {
    let error_info = ErrorInfo::from(&error);
    tpl.error_message = Some(error_info.message);

    Ok(Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)?)
}
