use snafu::{OptionExt, ResultExt, ensure};
use validator::Validate;

use crate::{
    Result,
    error::{CipherSnafu, DbSnafu, NotFoundSnafu, ValidationSnafu},
    state::AppState,
};
use cipher::encrypt;
use db::file::MAX_FILES;
use memo::{
    dir::DirDto,
    file::{FileDto, FileType},
    note::{CreateNoteDto, NoteDto},
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

    let file = state
        .db
        .files
        .retry_create(file, 5)
        .await
        .context(DbSnafu)?;

    let checksum = str_checksum(&data.content);

    // Encrypt content before storing
    let cipher_content =
        encrypt(&state.config.notes_master_key, &data.content).context(CipherSnafu)?;

    if let Err(error) = state
        .db
        .notes
        .retry_create_revision(file.id.clone(), cipher_content, checksum, 5)
        .await
        .context(DbSnafu)
    {
        // Delete the file entry since we cannot insert a note revision.
        state.db.files.delete(&file.id).await.context(DbSnafu)?;
        return Err(error);
    }

    Ok(file)
}

pub async fn get_note_svc(state: &AppState, file_id: &str) -> Result<Option<NoteDto>> {
    state.db.notes.get_note(file_id).await.context(DbSnafu)
}

pub async fn update_note_svc(
    state: &AppState,
    file_id: String,
    content: String,
) -> Result<NoteDto> {
    let content_length = content.len();
    let now = chrono::Utc::now().timestamp();

    // Ensure the same content is not being saved again.
    let note = state.db.notes.get_note(&file_id).await.context(DbSnafu)?;
    let note = note.context(NotFoundSnafu {
        msg: "Note not found".to_string(),
    })?;

    let checksum = str_checksum(&content);
    if note.checksum == checksum {
        // Return the existing note without creating a new revision
        return Ok(note);
    }

    // Encrypt content before storing
    let cipher_content = encrypt(&state.config.notes_master_key, &content).context(CipherSnafu)?;

    // Otherwise, create a new revision
    let new_revision = state
        .db
        .notes
        .retry_create_revision(file_id.clone(), cipher_content, checksum, 5)
        .await
        .context(DbSnafu)?;

    // Update file meta data
    state
        .db
        .files
        .update_notes_meta(&file_id, content_length as i64, now)
        .await
        .context(DbSnafu)?;

    Ok(new_revision)
}
