use askama::Template;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Extension, Form, body::Body, extract::State, response::Response};
use chrono::{DateTime, Utc};
use memo::dir::{DirDto, DirType};
use memo::file::FileDto;
use memo::pagination::PaginatedMeta;
use serde::Deserialize;
use snafu::ResultExt;
use urlencoding::encode;

use crate::models::{ListFilesParams, Pref, TemplateData, tokens::TokenFormData};
use crate::services::notes::{
    CreateNoteFormData, UpdateNoteFormData, create_note_svc, delete_note_svc, get_note_content_svc,
    list_notes_svc, update_note_svc,
};
use crate::services::token::create_csrf_token;
use crate::{
    Error, Result,
    ctx::Ctx,
    error::{ErrorInfo, ResponseBuilderSnafu, TemplateSnafu},
    run::AppState,
    web::{Action, Resource, enforce_policy},
};

#[derive(Deserialize)]
pub struct NoteRevisionQuery {
    revision: Option<String>,
}

impl NoteRevisionQuery {
    fn suffix(&self) -> String {
        self.revision
            .as_deref()
            .map(|revision| format!("?revision={}", encode(revision)))
            .unwrap_or_default()
    }
}

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
    revision_query: String,
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
    revision_query: String,
}

#[derive(Template)]
#[template(path = "pages/note.html")]
struct NotePageTemplate {
    t: TemplateData,
    dir: DirDto,
    note: FileDto,
    content: String,
    can_edit: bool,
    can_delete: bool,
    revision_query: String,
}

#[derive(Template)]
#[template(path = "widgets/edit_note_controls.html")]
struct EditNoteControlsTemplate {
    dir: DirDto,
    note: FileDto,
    can_edit: bool,
    can_delete: bool,
    revision_query: String,
}

#[derive(Template)]
#[template(path = "widgets/delete_note_form.html")]
struct DeleteNoteTemplate {
    dir: DirDto,
    note: FileDto,
    token: String,
    error_message: Option<String>,
    revision_query: String,
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
        revision_query: String::new(),
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
                revision_query: String::new(),
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
    Query(query): Query<NoteRevisionQuery>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Read)?;
    let token = ctx.token().expect("token is required");
    let content =
        get_note_content_svc(&state, token, &dir.id, &note.id, query.revision.as_deref()).await?;
    let mut t = TemplateData::new(&state, actor, &pref);
    t.title = format!("Notes - {}", note.name);
    let tpl = NotePageTemplate {
        t,
        dir,
        note,
        content: content.content,
        can_edit: enforce_policy(actor, Resource::File, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::File, Action::Delete).is_ok(),
        revision_query: query.suffix(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn edit_note_controls_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    Query(query): Query<NoteRevisionQuery>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Read)?;
    let tpl = EditNoteControlsTemplate {
        dir,
        note,
        can_edit: enforce_policy(actor, Resource::File, Action::Update).is_ok(),
        can_delete: enforce_policy(actor, Resource::File, Action::Delete).is_ok(),
        revision_query: query.suffix(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn get_delete_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
    Query(query): Query<NoteRevisionQuery>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    enforce_policy(ctx.actor(), Resource::File, Action::Delete)?;
    let tpl = DeleteNoteTemplate {
        token: create_csrf_token(&note.id, &state.config.jwt_secret)?,
        dir,
        note,
        error_message: None,
        revision_query: query.suffix(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(tpl.render().context(TemplateSnafu)?))
        .context(ResponseBuilderSnafu)
}

pub async fn post_delete_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
    Query(query): Query<NoteRevisionQuery>,
    Form(payload): Form<TokenFormData>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    enforce_policy(ctx.actor(), Resource::File, Action::Delete)?;
    let token = ctx.token().expect("token is required");
    match delete_note_svc(&state, token, &dir.id, &note.id, &payload.token).await {
        Ok(()) => Response::builder()
            .status(StatusCode::OK)
            .header("HX-Redirect", format!("/notes/{}", dir.id))
            .body(Body::empty())
            .context(ResponseBuilderSnafu),
        Err(err) => {
            let error = ErrorInfo::from(&err);
            let tpl = DeleteNoteTemplate {
                token: create_csrf_token(&note.id, &state.config.jwt_secret)?,
                dir,
                note,
                error_message: Some(error.message),
                revision_query: query.suffix(),
            };
            Response::builder()
                .status(error.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)
        }
    }
}

pub async fn edit_note_handler(
    Extension(ctx): Extension<Ctx>,
    Extension(pref): Extension<Pref>,
    Extension(dir): Extension<DirDto>,
    Extension(note): Extension<FileDto>,
    State(state): State<AppState>,
    Query(query): Query<NoteRevisionQuery>,
) -> Result<Response<Body>> {
    require_notes_dir(&dir)?;
    let actor = ctx.actor();
    enforce_policy(actor, Resource::File, Action::Update)?;
    let token = ctx.token().expect("token is required");
    let content =
        get_note_content_svc(&state, token, &dir.id, &note.id, query.revision.as_deref()).await?;
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
        revision_query: query.suffix(),
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
    Query(query): Query<NoteRevisionQuery>,
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
                revision_query: query.suffix(),
            };
            Response::builder()
                .status(error.status_code)
                .body(Body::from(tpl.render().context(TemplateSnafu)?))
                .context(ResponseBuilderSnafu)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memo::file::FileType;

    fn dir() -> DirDto {
        DirDto {
            id: "dir-id".into(),
            org_id: "org-id".into(),
            dir_type: DirType::Notes,
            name: "notes".into(),
            label: "Notes".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn note() -> FileDto {
        FileDto {
            id: "note-id".into(),
            org_id: "org-id".into(),
            dir_id: "dir-id".into(),
            file_type: FileType::Note,
            name: "My note".into(),
            filename: "note".into(),
            content_type: "text/plain".into(),
            size: 0,
            url: None,
            img_versions: None,
            img_taken_at: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn note_controls_respect_delete_and_edit_permissions() {
        let mut controls = EditNoteControlsTemplate {
            dir: dir(),
            note: note(),
            can_edit: false,
            can_delete: true,
            revision_query: String::new(),
        };
        let html = controls.render().unwrap();
        assert!(html.contains("hx-get=\"/notes/dir-id/entries/note-id/delete\""));
        assert!(html.contains("hx-target=\"#edit-note-container\""));
        assert!(html.contains("href=\"/notes/dir-id\""));
        assert!(html.contains("fa-arrow-left"));
        assert!(!html.contains("My note"));
        assert!(!html.contains("/edit\""));

        controls.can_edit = true;
        controls.can_delete = false;
        let html = controls.render().unwrap();
        assert!(html.contains("/notes/dir-id/entries/note-id/edit"));
        assert!(!html.contains(">Delete</button>"));
    }

    #[test]
    fn delete_note_confirmation_contains_token_cancel_and_error() {
        let form = DeleteNoteTemplate {
            dir: dir(),
            note: note(),
            token: "csrf-token".into(),
            error_message: Some("Unable to delete".into()),
            revision_query: String::new(),
        };
        let html = form.render().unwrap();
        assert!(html.contains("hx-post=\"/notes/dir-id/entries/note-id/delete\""));
        assert!(html.contains("hx-get=\"/notes/dir-id/entries/note-id/edit_controls\""));
        assert!(html.contains("hx-target=\"#edit-note-container\""));
        assert!(html.contains("value=\"csrf-token\""));
        assert!(html.contains("My note"));
        assert!(html.contains("Unable to delete"));
    }

    #[test]
    fn revision_query_is_encoded_and_preserved_in_fragments() {
        let query = NoteRevisionQuery {
            revision: Some("old &/revision".into()),
        };
        let suffix = query.suffix();
        assert_eq!(suffix, "?revision=old%20%26%2Frevision");
        assert_eq!(NoteRevisionQuery { revision: None }.suffix(), "");
        assert_eq!(
            NoteRevisionQuery {
                revision: Some(String::new())
            }
            .suffix(),
            "?revision="
        );

        let controls = EditNoteControlsTemplate {
            dir: dir(),
            note: note(),
            can_edit: true,
            can_delete: true,
            revision_query: suffix.clone(),
        }
        .render()
        .unwrap();
        assert!(controls.contains("/edit?revision=old%20%26%2Frevision"));
        assert!(controls.contains("/delete?revision=old%20%26%2Frevision"));

        let delete = DeleteNoteTemplate {
            dir: dir(),
            note: note(),
            token: "token".into(),
            error_message: Some("error".into()),
            revision_query: suffix.clone(),
        }
        .render()
        .unwrap();
        assert!(delete.contains("/delete?revision=old%20%26%2Frevision"));
        assert!(delete.contains("/edit_controls?revision=old%20%26%2Frevision"));
        assert!(delete.contains("/entries/note-id?revision=old%20%26%2Frevision"));

        let form = NoteFormTemplate {
            dir: dir(),
            name: "My note".into(),
            content: "old content".into(),
            token: "token".into(),
            error_message: Some("error".into()),
            is_edit: true,
            note_id: "note-id".into(),
            revision_query: suffix,
        }
        .render()
        .unwrap();
        assert!(form.contains("/edit?revision=old%20%26%2Frevision"));
        assert!(form.contains("/entries/note-id?revision=old%20%26%2Frevision"));
        assert!(form.contains("old content"));
    }

    #[test]
    fn note_title_is_outside_replaceable_controls() {
        let page = NotePageTemplate {
            t: TemplateData {
                theme: "light".into(),
                title: "Note".into(),
                assets: crate::config::AssetManifest {
                    main_js: String::new(),
                    main_css: String::new(),
                },
                styles: Vec::new(),
                scripts: Vec::new(),
                async_scripts: Vec::new(),
                script_vars: Vec::new(),
                ga_tag_id: None,
                actor: None,
            },
            dir: dir(),
            note: note(),
            content: "content".into(),
            can_edit: true,
            can_delete: true,
            revision_query: String::new(),
        }
        .render()
        .unwrap();

        let title = page.find("<h1 class=\"title\">My note</h1>").unwrap();
        let controls = page.find("id=\"edit-note-container\"").unwrap();
        assert!(title < controls);
        assert!(!page[controls..].contains("<h1"));
    }
}
