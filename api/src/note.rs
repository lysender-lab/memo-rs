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
    note::{CreateNoteDto, NoteDto, NoteRevisionDto, UpdateNoteDto},
    utils::{IdPrefix, generate_prefixed_id, str_checksum, truncate_string, valid_prefixed_id},
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

pub async fn get_note_svc(
    state: &AppState,
    file_id: &str,
    revision: Option<&str>,
) -> Result<Option<NoteDto>> {
    if let Some(revision_id) = revision {
        ensure!(
            revision_id.starts_with("not_") && valid_prefixed_id(revision_id),
            ValidationSnafu {
                msg: "Revision ID must be a valid note ID".to_string(),
            }
        );
    }

    let mut note = match revision {
        Some(revision_id) => state
            .db
            .notes
            .get_revision(file_id, revision_id)
            .await
            .context(DbSnafu)?,
        None => state.db.notes.get_note(file_id).await.context(DbSnafu)?,
    };
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

pub async fn list_note_revisions_svc(
    state: &AppState,
    file_id: &str,
) -> Result<Vec<NoteRevisionDto>> {
    let revisions = state
        .db
        .notes
        .list_revisions(file_id)
        .await
        .context(DbSnafu)?;

    Ok(revisions.into_iter().map(NoteRevisionDto::from).collect())
}

pub async fn delete_note_svc(state: &AppState, file_id: &str) -> Result<()> {
    state.db.files.delete_note(file_id).await.context(DbSnafu)?;

    state.file_cache.remove(file_id);

    Ok(())
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
        .retry_update_note(
            file_id.clone(),
            cipher_content,
            checksum,
            content_length as i64,
            now,
            5,
        )
        .await
        .context(DbSnafu)?;

    state.file_cache.remove(&file_id);

    Ok(new_revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn revision_list_json_exposes_latest_marker_without_internal_fields() {
        let note = |id: &str, next_revision: &str, created_at| NoteDto {
            id: id.to_string(),
            file_id: "file-id".to_string(),
            content: "encrypted content".to_string(),
            next_revision: next_revision.to_string(),
            checksum: "checksum".to_string(),
            created_at,
        };
        // List order is not used to determine which revision is latest.
        let revisions: Vec<NoteRevisionDto> =
            vec![note("older", "current", 2), note("current", "latest", 1)]
                .into_iter()
                .map(NoteRevisionDto::from)
                .collect();

        assert_eq!(
            serde_json::to_value(&revisions).unwrap(),
            json!([
                {"id": "older", "file_id": "file-id", "latest": false, "created_at": 2},
                {"id": "current", "file_id": "file-id", "latest": true, "created_at": 1}
            ])
        );
        assert_eq!(
            serde_json::to_value(Vec::<NoteRevisionDto>::new()).unwrap(),
            json!([])
        );
    }
}
