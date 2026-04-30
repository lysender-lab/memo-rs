use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, body::Body, extract::State, response::Response};
use axum::{Form, Json};
use chrono::{DateTime, Utc};
use memo::dir::DirType;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::ListFilesParams;
use crate::models::tokens::TokenFormData;
use crate::services::files::{
    CommitUploadPayload, Photo, PrepareUploadPayload, add_file_svc, delete_file_svc,
    list_any_files_svc, list_files_svc, prepare_upload_svc,
};
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    models::{Pref, TemplateData},
    run::AppState,
    services::token::create_csrf_token,
    web::{Action, Resource, enforce_policy},
};
use memo::dir::DirDto;
use memo::file::SignedFileUploadDto;
use memo::pagination::PaginatedMeta;

use super::document_whitelist::{document_accept_attr, document_allowed_exts_attr};
use super::handle_error_message;

#[derive(Template)]
#[template(path = "widgets/photo_grid.html")]
struct PhotoGridTemnplate {
    theme: String,
    dir: DirDto,
    photos: Vec<Photo>,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    last_item: String,
}

#[derive(Clone)]
struct FileRowView {
    id: String,
    name: String,
    size_text: String,
    uploaded_at_text: String,
    icon_class: String,
    url: Option<String>,
}

#[derive(Template)]
#[template(path = "widgets/file_table.html")]
struct FileTableTemplate {
    dir: DirDto,
    files: Vec<FileRowView>,
    has_files: bool,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    keyword_query: String,
    last_item: String,
}

fn format_file_size(size: i64) -> String {
    if size < 1024 {
        return format!("{} B", size);
    }

    let kb = size as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }

    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }

    let gb = mb / 1024.0;
    format!("{gb:.1} GB")
}

fn format_uploaded_at(ts: i64) -> String {
    let Some(dt) = DateTime::from_timestamp(ts, 0) else {
        return "-".to_string();
    };

    dt.with_timezone(&Utc)
        .format("%Y-%m-%d %H:%M UTC")
        .to_string()
}

fn file_icon_class(name: &str, content_type: &str) -> &'static str {
    let lower = name.to_lowercase();

    if content_type == "application/pdf" || lower.ends_with(".pdf") {
        return "fa-file-pdf";
    }

    if content_type.contains("word") || lower.ends_with(".doc") || lower.ends_with(".docx") {
        return "fa-file-word";
    }

    if content_type.contains("sheet") || lower.ends_with(".xls") || lower.ends_with(".xlsx") {
        return "fa-file-excel";
    }

    if content_type.contains("presentation") || lower.ends_with(".ppt") || lower.ends_with(".pptx")
    {
        return "fa-file-powerpoint";
    }

    if content_type.starts_with("text/") || lower.ends_with(".txt") || lower.ends_with(".md") {
        return "fa-file-lines";
    }

    if content_type.contains("zip")
        || lower.ends_with(".zip")
        || lower.ends_with(".rar")
        || lower.ends_with(".7z")
        || lower.ends_with(".tar")
        || lower.ends_with(".gz")
    {
        return "fa-file-zipper";
    }

    if content_type.starts_with("audio/") {
        return "fa-file-audio";
    }

    if content_type.starts_with("video/") {
        return "fa-file-video";
    }

    if content_type.starts_with("image/") {
        return "fa-file-image";
    }

    "fa-file"
}

pub async fn file_table_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Query(query): Query<ListFilesParams>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Read)?;

    let dir_id = dir.id.clone();
    let dir_type = dir.dir_type.clone();

    let mut tpl = FileTableTemplate {
        dir,
        files: Vec::new(),
        has_files: false,
        meta: None,
        error_message: None,
        next_page: None,
        keyword_query: encode(query.keyword.as_deref().unwrap_or("")).into_owned(),
        last_item: "".to_string(),
    };

    let auth_token = ctx.token().expect("token is required");
    let result = list_any_files_svc(&state, auth_token, &dir_type, &dir_id, &query).await;

    match result {
        Ok(listing) => {
            tpl.files = listing
                .data
                .iter()
                .map(|file| FileRowView {
                    id: file.id.clone(),
                    name: file.name.clone(),
                    size_text: format_file_size(file.size),
                    uploaded_at_text: format_uploaded_at(file.created_at),
                    icon_class: file_icon_class(&file.name, &file.content_type).to_string(),
                    url: file.url.clone(),
                })
                .collect();
            tpl.has_files = !tpl.files.is_empty();

            if listing.meta.total_pages > listing.meta.page as i64 {
                tpl.next_page = Some(listing.meta.page as i64 + 1);
            }

            if let Some(file) = tpl.files.last() {
                tpl.last_item = file.id.clone();
            }
            tpl.meta = Some(listing.meta);

            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
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

pub async fn photo_listing_v2_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    Query(query): Query<ListFilesParams>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Read)?;

    let dir_id = dir.id.clone();
    let dir_type = dir.dir_type.clone();

    let mut tpl = PhotoGridTemnplate {
        theme: pref.theme,
        dir: dir,
        photos: Vec::new(),
        meta: None,
        error_message: None,
        next_page: None,
        last_item: "".to_string(),
    };

    let auth_token = ctx.token().expect("token is required");
    let result = list_files_svc(&state, auth_token, &dir_type, &dir_id, &query).await;

    match result {
        Ok(listing) => {
            tpl.photos = listing.data;

            if listing.meta.total_pages > listing.meta.page as i64 {
                tpl.next_page = Some(listing.meta.page as i64 + 1);
            }

            // Get the last item
            if let Some(photo) = tpl.photos.last() {
                tpl.last_item = photo.id.clone();
            }
            tpl.meta = Some(listing.meta);

            Ok(Response::builder()
                .status(200)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
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

#[derive(Template)]
#[template(path = "pages/upload_photos.html")]
struct UploadPhotosPageTemplate {
    t: TemplateData,
    dir: DirDto,
    token: String,
}

#[derive(Template)]
#[template(path = "pages/upload_documents.html")]
struct UploadDocumentsPageTemplate {
    t: TemplateData,
    dir: DirDto,
    token: String,
    accept_attr: String,
    allowed_exts_attr: String,
}

#[derive(Template)]
#[template(path = "widgets/photo_grid_item.html")]
struct UploadedPhotoTemplate {
    theme: String,
    photo: Photo,
}

#[derive(Template)]
#[template(path = "widgets/uploaded_file_item.html")]
struct UploadedFileTemplate {
    name: String,
    icon_class: String,
    url: Option<String>,
}

pub async fn upload_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();

    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Create)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;
    let mut t = TemplateData::new(&state, actor, &pref);

    let body = if dir.dir_type == DirType::Documents {
        t.title = format!("Documents - {} - Upload Files", &dir.label);
        let tpl = UploadDocumentsPageTemplate {
            t,
            dir,
            token,
            accept_attr: document_accept_attr(),
            allowed_exts_attr: document_allowed_exts_attr(),
        };

        tpl.render().context(TemplateSnafu)?
    } else {
        t.title = format!("Photos - {} - Upload Photos", &dir.label);
        let tpl = UploadPhotosPageTemplate { t, dir, token };

        tpl.render().context(TemplateSnafu)?
    };

    Response::builder()
        .status(200)
        .body(Body::from(body))
        .context(ResponseBuilderSnafu)
}

pub async fn generate_upload_url_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Json<PrepareUploadPayload>,
) -> Result<(StatusCode, Json<SignedFileUploadDto>)> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Create)?;

    let auth_token = ctx.token().expect("token is required");
    let dto = prepare_upload_svc(&state, auth_token, &dir.dir_type, &dir.id, payload.0).await?;

    Ok((StatusCode::OK, Json(dto)))
}

pub async fn add_file_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    payload: Form<CommitUploadPayload>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Create)?;

    let token = create_csrf_token(&dir.id, &config.jwt_secret)?;

    let auth_token = ctx.token().expect("token is required");
    let result = add_file_svc(&state, auth_token, &dir.dir_type, &dir.id, payload.0).await;

    match result {
        Ok(file) => {
            let html = if dir.dir_type == DirType::Documents {
                let name = file.name;
                let icon_class = file_icon_class(&name, &file.content_type).to_string();
                let tpl = UploadedFileTemplate {
                    name,
                    icon_class,
                    url: file.url,
                };

                tpl.render().context(TemplateSnafu)?
            } else {
                let photo: Photo = Photo::try_from(file).map_err(|msg| Error::Whatever { msg })?;
                let tpl = UploadedPhotoTemplate {
                    photo,
                    theme: pref.theme,
                };

                tpl.render().context(TemplateSnafu)?
            };

            Ok(Response::builder()
                .status(201)
                .header("X-Next-Token", token)
                .body(Body::from(html))
                .context(ResponseBuilderSnafu)?)
        }
        Err(err) => Ok(handle_error_message(&err)),
    }
}

#[derive(Template)]
#[template(path = "widgets/pre_delete_photo_form.html")]
struct PreDeletePhotoTemplate {
    dir: DirDto,
    photo: Photo,
}

#[derive(Template)]
#[template(path = "widgets/confirm_delete_photo_form.html")]
struct ConfirmDeletePhotoTemplate {
    dir: DirDto,
    photo: Photo,
    payload: TokenFormData,
    error_message: Option<String>,
}

/// Shows pre-delete form controls
pub async fn pre_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
) -> Result<Response<Body>> {
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    // Just render the form on first load or on error
    let tpl = PreDeletePhotoTemplate { dir, photo };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

/// Shows delete/cancel form controls
pub async fn confirm_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        let error = Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        };
        return Ok(handle_error_message(&error));
    };

    // Just render the form on first load or on error
    let tpl = ConfirmDeletePhotoTemplate {
        dir,
        photo,
        payload: TokenFormData { token },
        error_message: None,
    };

    Response::builder()
        .status(200)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn exec_delete_photo_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(photo): Extension<Photo>,
    State(state): State<AppState>,
    payload: Form<TokenFormData>,
) -> Result<Response<Body>> {
    let config = state.config.clone();
    let actor = ctx.actor();
    let dir_id = dir.id.clone();
    let dir_type = dir.dir_type.clone();

    if let Err(err) = enforce_policy(actor, Resource::Photo, Action::Delete) {
        return Ok(handle_error_message(&err));
    }

    let Ok(token) = create_csrf_token(&photo.id, &config.jwt_secret) else {
        return Ok(handle_error_message(&Error::Whatever {
            msg: "Failed to initialize delete photo form.".to_string(),
        }));
    };

    let auth_token = ctx.token().expect("token is required");
    let result = delete_file_svc(
        &state,
        auth_token,
        &dir_type,
        &dir_id,
        &photo.id,
        &payload.token,
    )
    .await;
    match result {
        Ok(_) => Response::builder()
            .status(204)
            .header("HX-Trigger", "PhotoDeletedEvent")
            .body(Body::from("".to_string()))
            .context(ResponseBuilderSnafu),
        Err(err) => {
            let error_info = ErrorInfo::from(&err);

            // Re-render the form with a new token
            // We may need to render an error message somewhere in the page
            let tpl = ConfirmDeletePhotoTemplate {
                dir,
                photo,
                payload: TokenFormData { token },
                error_message: Some(error_info.message),
            };

            Ok(Response::builder()
                .status(error_info.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)?)
        }
    }
}
