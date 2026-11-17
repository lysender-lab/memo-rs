use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use chrono::{DateTime, Utc};
use memo::dir::{DirDto, DirType};
use memo::file::FileDto;
use memo::pagination::PaginatedMeta;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::{ListFilesParams, Pref, TemplateData};
use crate::services::notes::{
    CreateNoteFormData, UpdateNoteFormData, create_note_svc, get_note_content_svc, list_notes_svc,
    update_note_svc,
};
use crate::services::token::create_csrf_token;
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    run::AppState,
    web::{Action, Resource, enforce_policy},
};

#[derive(Clone)]
struct NoteRowView {
    id: String,
    name: String,
    size_text: String,
    updated_at_text: String,
}

#[derive(Template)]
#[template(path = "widgets/note_table.html")]
struct NoteListingTemplate {
    dir: DirDto,
    notes: Vec<NoteRowView>,
    has_notes: bool,
    meta: Option<PaginatedMeta>,
    error_message: Option<String>,
    next_page: Option<i64>,
    keyword_query: String,
    last_item: String,
}

#[derive(Template)]
#[template(path = "pages/note_form.html")]
struct NoteFormPageTemplate {
    t: TemplateData,
    dir: DirDto,
    name: String,
    content: String,
    token: String,
    error_message: Option<String>,
    is_edit: bool,
    note_id: String,
}

#[derive(Template)]
#[template(path = "widgets/note_form.html")]
struct NoteFormTemplate {
    dir: DirDto,
    name: String,
    content: String,
    token: String,
    error_message: Option<String>,
    is_edit: bool,
    note_id: String,
}

#[derive(Template)]
#[template(path = "pages/note.html")]
struct NotePageTemplate {
    t: TemplateData,
    dir: DirDto,
    note: FileDto,
    content: String,
    can_edit: bool,
}

fn require_notes_dir(dir: &DirDto) -> Result<()> {
    if dir.dir_type == DirType::Notes {
        Ok(())
    } else {
        Err(Error::FileNotFound)
    }
}

fn format_file_size(size: i64) -> String {
    if size < 1024 {
        return format!("{} B", size);
    }
    let kb = size as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    format!("{:.1} MB", kb / 1024.0)
}

fn format_updated_at(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|| "-".to_string())
}

pub async fn note_listing_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    Query(query): Query<ListFilesParams>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Read)?;

    let mut tpl = NoteListingTemplate {
        dir: dir.clone(),
        notes: Vec::new(),
        has_notes: false,
        meta: None,
        error_message: None,
        next_page: None,
        keyword_query: encode(query.keyword.as_deref().unwrap_or("")).into_owned(),
        last_item: String::new(),
    };
    let mut status = StatusCode::OK;
    let token = ctx.token().expect("token is required");
    match list_notes_svc(&state, token, &dir.id, &query).await {
        Ok(listing) => {
            tpl.notes = listing
                .data
                .into_iter()
                .map(|note| NoteRowView {
                    id: note.id,
                    name: note.name,
                    size_text: format_file_size(note.size),
                    updated_at_text: format_updated_at(note.updated_at),
                })
                .collect();
            tpl.has_notes = !tpl.notes.is_empty();
            if listing.meta.total_pages > listing.meta.page as i64 {
                tpl.next_page = Some(listing.meta.page as i64 + 1);
            }
            if let Some(note) = tpl.notes.last() {
                tpl.last_item = note.id.clone();
            }
            tpl.meta = Some(listing.meta);
        }
        Err(err) => {
            let error = ErrorInfo::from(&err);
            status = error.status_code;
            tpl.error_message = Some(error.message);
        }
    }

    Response::builder()
        .status(status)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn new_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Create)?;
    let token = create_csrf_token("new_note", &state.config.jwt_secret)?;
    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = format!("Notes - {} - New Note", dir.label);
    let tpl = NoteFormPageTemplate {
        t,
        dir,
        name: String::new(),
        content: String::new(),
        token,
        error_message: None,
        is_edit: false,
        note_id: String::new(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_new_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    State(state): State<AppState>,
    Form(payload): Form<CreateNoteFormData>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Create)?;
    let token = ctx.token().expect("token is required");
    match create_note_svc(&state, token, &dir.id, &payload).await {
        Ok(_) => Response::builder()
            .status(StatusCode::OK)
            .header("HX-Redirect", format!("/notes/{}", dir.id))
            .body(Body::empty())
            .context(ResponseBuilderSnafu),
        Err(err) => {
            let error = ErrorInfo::from(&err);
            let tpl = NoteFormTemplate {
                dir,
                name: payload.name,
                content: payload.content,
                token: create_csrf_token("new_note", &state.config.jwt_secret)?,
                error_message: Some(error.message),
                is_edit: false,
                note_id: String::new(),
            };
            Response::builder()
                .status(error.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)
        }
    }
}

pub async fn note_page_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Read)?;
    let token = ctx.token().expect("token is required");
    let content = get_note_content_svc(&state, token, &dir.id, &note.id).await?;
    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = format!("Notes - {}", note.name);
    let tpl = NotePageTemplate {
        t,
        dir,
        note,
        content: content.content,
        can_edit: enforce_policy(actor, Resource::File, Action::Update).is_ok(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn edit_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Update)?;
    let token = ctx.token().expect("token is required");
    let content = get_note_content_svc(&state, token, &dir.id, &note.id).await?;
    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = format!("Notes - {} - Edit", note.name);
    let tpl = NoteFormPageTemplate {
        t,
        dir,
        name: note.name,
        content: content.content,
        token: create_csrf_token(&note.id, &state.config.jwt_secret)?,
        error_message: None,
        is_edit: true,
        note_id: note.id,
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_edit_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
    Form(payload): Form<UpdateNoteFormData>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Update)?;
    let token = ctx.token().expect("token is required");
    match update_note_svc(&state, token, &dir.id, &note.id, &payload).await {
        Ok(_) => Response::builder()
            .status(StatusCode::OK)
            .header("HX-Redirect", format!("/notes/{}", dir.id))
            .body(Body::empty())
            .context(ResponseBuilderSnafu),
        Err(err) => {
            let error = ErrorInfo::from(&err);
            let tpl = NoteFormTemplate {
                dir,
                name: note.name,
                content: payload.content,
                token: create_csrf_token(&note.id, &state.config.jwt_secret)?,
                error_message: Some(error.message),
                is_edit: true,
                note_id: note.id,
            };
            Response::builder()
                .status(error.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)
        }
    }
}
