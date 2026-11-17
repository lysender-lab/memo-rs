use memo::file::FileDto;
use memo::note::NoteContentDto;
use memo::pagination::Paginated;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::ListFilesParams;
use crate::run::AppState;
use crate::services::handle_response_error;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

#[derive(Clone, Deserialize)]
pub struct CreateNoteFormData {
    pub name: String,
    pub content: String,
    pub token: String,
}

#[derive(Clone, Deserialize)]
pub struct UpdateNoteFormData {
    pub content: String,
    pub token: String,
}

#[derive(Serialize)]
struct CreateNoteData {
    name: String,
    content: String,
}

#[derive(Serialize)]
struct UpdateNoteData {
    content: String,
}

pub async fn list_notes_svc(
    state: &AppState,
    token: &str,
    dir_id: &str,
    params: &ListFilesParams,
) -> Result<Paginated<FileDto>> {
    let url = format!("{}/notes/{}/entries", state.config.api_url, dir_id);
    let page = params.page.unwrap_or(1).to_string();
    let per_page = "50".to_string();
    let keyword = params.keyword.clone().unwrap_or_default();
    let query = [
        ("page", page.as_str()),
        ("per_page", per_page.as_str()),
        ("keyword", keyword.as_str()),
    ];

    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list notes. Try again later.".to_string(),
        })?;
    if !response.status().is_success() {
        return Err(handle_response_error(response, "notes", Error::AlbumNotFound).await);
    }

    response
        .json::<Paginated<FileDto>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse notes.".to_string(),
        })
}

pub async fn get_note_file_svc(
    state: &AppState,
    token: &str,
    dir_id: &str,
    note_id: &str,
) -> Result<FileDto> {
    let url = format!(
        "{}/notes/{}/entries/{}",
        state.config.api_url, dir_id, note_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get note. Try again later.".to_string(),
        })?;
    if !response.status().is_success() {
        return Err(handle_response_error(response, "note", Error::FileNotFound).await);
    }

    response
        .json::<FileDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse note information.".to_string(),
        })
}

pub async fn get_note_content_svc(
    state: &AppState,
    token: &str,
    dir_id: &str,
    note_id: &str,
) -> Result<NoteContentDto> {
    let url = format!(
        "{}/notes/{}/entries/{}/content",
        state.config.api_url, dir_id, note_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get note content. Try again later.".to_string(),
        })?;
    if !response.status().is_success() {
        return Err(handle_response_error(response, "note", Error::FileNotFound).await);
    }

    response
        .json::<NoteContentDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse note content.".to_string(),
        })
}

pub async fn create_note_svc(
    state: &AppState,
    token: &str,
    dir_id: &str,
    form: &CreateNoteFormData,
) -> Result<FileDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == "new_note", CsrfTokenSnafu);

    let url = format!("{}/notes/{}/entries", state.config.api_url, dir_id);
    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&CreateNoteData {
            name: form.name.clone(),
            content: form.content.clone(),
        })
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create note. Try again later.".to_string(),
        })?;
    if !response.status().is_success() {
        return Err(handle_response_error(response, "note", Error::FileNotFound).await);
    }

    response
        .json::<FileDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse note information.".to_string(),
        })
}

pub async fn update_note_svc(
    state: &AppState,
    token: &str,
    dir_id: &str,
    note_id: &str,
    form: &UpdateNoteFormData,
) -> Result<NoteContentDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == note_id, CsrfTokenSnafu);

    let url = format!(
        "{}/notes/{}/entries/{}/content",
        state.config.api_url, dir_id, note_id
    );
    let response = state
        .client
        .put(url)
        .bearer_auth(token)
        .json(&UpdateNoteData {
            content: form.content.clone(),
        })
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update note. Try again later.".to_string(),
        })?;
    if !response.status().is_success() {
        return Err(handle_response_error(response, "note", Error::FileNotFound).await);
    }

    state.file_cache.remove(note_id);
    response
        .json::<NoteContentDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse note content.".to_string(),
        })
}
