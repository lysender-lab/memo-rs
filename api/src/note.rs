use snafu::{OptionExt, ResultExt, ensure};
use validator::Validate;

use crate::{
    Result,
    error::{CipherSnafu, DbSnafu, NotFoundSnafu, ValidationSnafu},
    state::AppState,
};
use cipher::{decrypt, encrypt};
use db::file::MAX_FILES;
use memo::{
    dir::DirDto,
    file::{FileDto, FileType},
    note::{CreateNoteDto, NoteDto, UpdateNoteDto},
    utils::{IdPrefix, generate_prefixed_id, str_checksum, truncate_string},
    validators::flatten_errors,
};

pub async fn create_note_svc(
    state: &AppState,
    dir: &DirDto,
    data: &CreateNoteDto,
) -> Result<FileDto> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    let count = state
        .db
        .files
        .count_by_dir(&dir.id)
        .await
        .context(DbSnafu)?;

    ensure!(
        count < MAX_FILES as i64,
        ValidationSnafu {
            msg: "Directory already has maximum files".to_string(),
        }
    );

    let existing = state
        .db
        .files
        .find_by_name(&dir.id, &data.name)
        .await
        .context(DbSnafu)?;

    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: format!("{} already exists", truncate_string(&data.name, 20)),
        }
    );

    let now = chrono::Utc::now().timestamp();

    let file = FileDto {
        id: generate_prefixed_id(IdPrefix::File),
        org_id: dir.org_id.clone(),
        dir_id: dir.id.clone(),
        file_type: FileType::Note,
        name: data.name.clone(),
        filename: data.name.clone(),
        content_type: "text/plain".to_string(),
        size: data.content.len() as i64,
        url: None,
        img_versions: None,
        img_taken_at: None,
        created_at: now,
        updated_at: now,
    };

    let checksum = str_checksum(&data.content);

    // Encrypt content before storing
    let cipher_content =
        encrypt(&state.config.notes_master_key, &data.content).context(CipherSnafu)?;

    state
        .db
        .files
        .retry_create_note(file, cipher_content, checksum, 5)
        .await
        .context(DbSnafu)
}

pub async fn get_note_svc(state: &AppState, file_id: &str) -> Result<Option<NoteDto>> {
    let mut note = state.db.notes.get_note(file_id).await.context(DbSnafu)?;
    if let Some(note_val) = note.as_mut() {
        // Decrypt and replace content
        let crypt_content = note_val.content.clone();
        let plain_content =
            decrypt(&state.config.notes_master_key, &&crypt_content).context(CipherSnafu)?;
        note_val.content = plain_content;

        return Ok(Some(note_val.to_owned()));
    }

    Ok(note)
}

pub async fn update_note_svc(
    state: &AppState,
    file_id: String,
    data: UpdateNoteDto,
) -> Result<NoteDto> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    let content_length = data.content.len();
    let now = chrono::Utc::now().timestamp();

    // Ensure the same content is not being saved again.
    let note = state.db.notes.get_note(&file_id).await.context(DbSnafu)?;
    let note = note.context(NotFoundSnafu {
        msg: "Note not found".to_string(),
    })?;

    let checksum = str_checksum(&data.content);
    if note.checksum == checksum {
        // Return the existing note without creating a new revision
        return Ok(note);
    }

    // Encrypt content before storing
    let cipher_content =
        encrypt(&state.config.notes_master_key, &data.content).context(CipherSnafu)?;

    // Create the revision and update the file metadata atomically.
    let new_revision = state
        .db
        .files
        .update_note(
            file_id.clone(),
            cipher_content,
            checksum,
            content_length as i64,
            now,
        )
        .await
        .context(DbSnafu)?;

    state.file_cache.remove(&file_id);

    Ok(new_revision)
}
