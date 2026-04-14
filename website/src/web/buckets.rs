use askama::Template;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use memo::bucket::BucketDto;
use snafu::ResultExt;
use yaas::role::Permission;

use crate::Error;
use crate::models::tokens::TokenFormData;
use crate::services::buckets::{
    NewBucketFormData, UpdateBucketFormData, create_bucket, delete_bucket, list_buckets,
    update_bucket,
};
use crate::{
    Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
    web::{Action, Resource, enforce_policy},
};

#[derive(Template)]
#[template(path = "pages/buckets.html")]
struct BucketsPageTemplate {
    t: TemplateData,
    buckets: Vec<BucketDto>,
}

pub async fn buckets_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Bucket, Action::Read)?;

    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from("Buckets");

    let token = ctx.token().expect("token is required");
    let buckets = list_buckets(&state, token).await?;

    let tpl = BucketsPageTemplate { t, buckets };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "pages/new_bucket.html")]
struct NewBucketTemplate {
    t: TemplateData,
    payload: NewBucketFormData,
    error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/new_bucket_form.html")]
struct NewBucketFormTemplate {
    payload: NewBucketFormData,
    error_message: Option<String>,
}

pub async fn new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = String::from("Create New Bucket");

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;

    let tpl = NewBucketTemplate {
        t,
        payload: NewBucketFormData {
            name: "".to_string(),
            label: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_new_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    State(state): State<AppState>,
    payload: Form<NewBucketFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Create)?;

    let token = create_csrf_token("new_bucket", &config.jwt_secret)?;

    let mut tpl = NewBucketFormTemplate {
        payload: NewBucketFormData {
            name: "".to_string(),
            label: "".to_string(),
            images_only: None,
            token,
        },
        error_message: None,
    };

    let bucket = NewBucketFormData {
        name: payload.name.clone(),
        label: payload.label.clone(),
        images_only: payload.images_only.clone(),
        token: payload.token.clone(),
    };

    let token = ctx.token().expect("token is required");
    let result = create_bucket(&state, token, &bucket).await;

    match result {
        Ok(_) => {
            let next_url = "/buckets".to_string();
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
    bucket: BucketDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
}

pub async fn bucket_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    let mut t = TemplateData::new(&state, actor, &pref);

    t.title = format!("Bucket - {}", &bucket.name);

    let tpl = BucketPageTemplate {
        t,
        bucket,
        updated: false,
        can_edit: actor.has_permissions(&vec![Permission::BucketsEdit]),
        can_delete: actor.has_permissions(&vec![Permission::BucketsEdit]),
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "widgets/edit_bucket_controls.html")]
struct BucketControlsTemplate {
    bucket: BucketDto,
    updated: bool,
    can_edit: bool,
    can_delete: bool,
}

pub async fn bucket_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Update)?;

    let tpl = BucketControlsTemplate {
        bucket,
        updated: false,
        can_edit: actor.has_permissions(&vec![Permission::BucketsEdit]),
        can_delete: actor.has_permissions(&vec![Permission::BucketsEdit]),
    };

    Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

#[derive(Template)]
#[template(path = "widgets/edit_bucket_form.html")]
struct EditBucketFormTemplate {
    payload: UpdateBucketFormData,
    bucket: BucketDto,
    error_message: Option<String>,
}

/// Renders the edit bucket form
pub async fn edit_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Update)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let label = bucket.label.clone();
    let tpl = EditBucketFormTemplate {
        bucket,
        payload: UpdateBucketFormData { label, token },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

/// Handles the edit album submission
pub async fn post_edit_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<UpdateBucketFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let bid = bucket.id.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Update)?;

    let token = create_csrf_token(&bid, &config.jwt_secret)?;

    let mut tpl = EditBucketFormTemplate {
        bucket: bucket.clone(),
        payload: UpdateBucketFormData {
            label: "".to_string(),
            token,
        },
        error_message: None,
    };

    tpl.payload.label = payload.label.clone();

    let token = ctx.token().expect("token is required");
    let result = update_bucket(&state, token, &bid, &payload).await;
    match result {
        Ok(updated_bucket) => {
            // Render the controls again with an out-of-bound swap for title
            let tpl = BucketControlsTemplate {
                bucket: updated_bucket,
                updated: true,
                can_edit: enforce_policy(actor, Resource::Bucket, Action::Update).is_ok(),
                can_delete: enforce_policy(actor, Resource::Bucket, Action::Delete).is_ok(),
            };
            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => {
            let status;
            match err {
                Error::Validation { msg } => {
                    status = 400;
                    tpl.error_message = Some(msg);
                }
                Error::LoginRequired => {
                    status = 401;
                    tpl.error_message = Some("Login required.".to_string());
                }
                any_err => {
                    status = 500;
                    tpl.error_message = Some(any_err.to_string());
                }
            }

            Ok(Response::builder()
                .status(status)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}

#[derive(Template)]
#[template(path = "widgets/delete_bucket_form.html")]
struct DeleteBucketFormTemplate {
    bucket: BucketDto,
    payload: TokenFormData,
    error_message: Option<String>,
}

pub async fn delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let tpl = DeleteBucketFormTemplate {
        bucket,
        payload: TokenFormData { token },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_delete_bucket_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(bucket): Extension<BucketDto>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    enforce_policy(actor, Resource::Bucket, Action::Delete)?;

    let token = create_csrf_token(&bucket.id, &config.jwt_secret)?;

    let mut tpl = DeleteBucketFormTemplate {
        bucket: bucket.clone(),
        payload: TokenFormData { token },
        error_message: None,
    };

    let token = ctx.token().expect("token is required");
    let result = delete_bucket(&state, token, &bucket.id, &payload.token).await;

    match result {
        Ok(_) => {
            // Render same form but trigger a redirect to home
            let tpl = DeleteBucketFormTemplate {
                bucket,
                payload: TokenFormData {
                    token: "".to_string(),
                },
                error_message: None,
            };
            Response::builder()
                .status(200)
                .header("HX-Redirect", "/buckets")
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)
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
