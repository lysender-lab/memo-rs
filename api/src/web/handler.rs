use core::result::Result as CoreResult;

use axum::{
    Extension,
    extract::{Json, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use snafu::{OptionExt, ResultExt, ensure};

use crate::{
    dir::{create_dir_svc, delete_dir_svc, update_dir_svc},
    error::{
        DbSnafu, ErrorResponse, ForbiddenSnafu, JsonRejectionSnafu, NotFoundSnafu, Result,
        StorageSnafu, WhateverSnafu,
    },
    file::{create_file_svc, create_remote_file_svc, delete_file_svc, generate_upload_url_svc},
    health::{check_liveness, check_readiness},
    note::{create_note_svc, get_note_svc, update_note_svc},
    state::AppState,
    token::verify_upload_token,
    web::response::JsonResponse,
};
use db::dir::{ListDirsParams, NewDir, UpdateDir};
use db::file::ListFilesParams;
use memo::{
    dir::{DirDto, DirMeta, DirType},
    file::{FileDto, ORIGINAL_PATH, RemoteUploadDto, SignedRemoteUploadDto},
    note::{CreateNoteDto, NoteContentDto, UpdateNoteDto},
    pagination::Paginated,
};
use yaas::{actor::Actor, role::Permission};

#[derive(Serialize)]
pub struct AppMeta {
    pub name: String,
    pub version: String,
}

pub async fn home_handler() -> impl IntoResponse {
    Json(AppMeta {
        name: "memo-rs".to_string(),
        version: "0.1.0".to_string(),
    })
}

pub async fn not_found_handler(State(_state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            status_code: StatusCode::NOT_FOUND.as_u16(),
            message: "Not Found",
            error: "Not Found",
        }),
    )
}

pub async fn health_live_handler() -> Result<JsonResponse> {
    let health = check_liveness().await?;
    Ok(JsonResponse::new(serde_json::to_string(&health).unwrap()))
}

pub async fn health_ready_handler(State(state): State<AppState>) -> Result<JsonResponse> {
    let health = check_readiness(&state.config, state.db).await?;
    let status = if health.is_healthy() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    Ok(JsonResponse::with_status(
        status,
        serde_json::to_string(&health).unwrap(),
    ))
}

pub async fn list_dirs_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir_type): Extension<DirType>,
    query: Query<ListDirsParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let actor = actor.actor.expect("Actor must be present");

    let dirs = state
        .db
        .dirs
        .list(&actor.org_id, &dir_type, &query)
        .await
        .context(DbSnafu)?;

    Ok(JsonResponse::new(serde_json::to_string(&dirs).unwrap()))
}

pub async fn create_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir_type): Extension<DirType>,
    payload: CoreResult<Json<NewDir>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsCreate];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let actor = actor.actor.expect("Actor must be present");

    let dir = create_dir_svc(&state, &actor.org_id, &dir_type, &data).await?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&dir).unwrap(),
    ))
}

pub async fn get_dir_handler(Extension(dir): Extension<DirDto>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap()))
}

pub async fn update_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    payload: CoreResult<Json<UpdateDir>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let updated = update_dir_svc(&state, &dir.id, &data).await?;

    // Either return the updated dir or the original one
    match updated {
        true => get_dir_as_response(&state, &dir.id).await,
        false => Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap())),
    }
}

async fn get_dir_as_response(state: &AppState, id: &str) -> Result<JsonResponse> {
    let res = state.db.dirs.get(id).await.context(DbSnafu)?;
    let dir = res.context(WhateverSnafu {
        msg: "Error getting directory",
    })?;

    Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap()))
}

pub async fn delete_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    delete_dir_svc(&state, &dir.id).await?;
    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn list_files_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    query: Query<ListFilesParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Ensure we are at non-notes dir
    ensure!(
        dir.dir_type != DirType::Notes,
        ForbiddenSnafu {
            msg: "Files are not allowed on notes"
        }
    );

    let files = state.db.files.list(&dir, &query).await.context(DbSnafu)?;

    let storage_client = state.storage_client.clone();

    let actor = actor.actor.expect("Actor must be present");

    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket.clone(),
        org_id: actor.org_id,
        dir_type: dir.dir_type,
        dir_name: dir.name,
    };

    // Generate download urls for each files
    let items = storage_client
        .attach_urls(&dir_meta, files.data)
        .await
        .context(StorageSnafu)?;

    let listing = Paginated::new(
        items,
        files.meta.page,
        files.meta.per_page,
        files.meta.total_records,
    );

    Ok(JsonResponse::new(serde_json::to_string(&listing).unwrap()))
}

/// Creates a pre-signed upload URL for a file.
pub async fn create_upload_url_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    payload: CoreResult<Json<RemoteUploadDto>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesCreate];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Do not allow uploads on notes
    ensure!(
        dir.dir_type != DirType::Notes,
        ForbiddenSnafu {
            msg: "Uploads are not allowed on notes"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let dto = generate_upload_url_svc(state, &dir, &data.0).await?;

    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn create_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    payload: CoreResult<Json<SignedRemoteUploadDto>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesCreate];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Ensure we are at non-notes dir
    ensure!(
        dir.dir_type != DirType::Notes,
        ForbiddenSnafu {
            msg: "Files are not allowed on notes"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    // Validate token
    let upload_claims = verify_upload_token(&data.token, &state.config.jwt_secret)?;
    let upload_claims_copy = upload_claims.clone();

    let is_image = upload_claims.is_image();
    let orig_filename = upload_claims.orig_filename;
    let new_filename = upload_claims.new_filename;
    let content_type = upload_claims.content_type;

    let actor = actor.actor.expect("Actor must be present");
    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket.clone(),
        org_id: actor.org_id,
        dir_type: dir.dir_type.clone(),
        dir_name: dir.name.clone(),
    };

    let storage_client = state.storage_client.clone();

    let file = if is_image {
        // Download file locally
        let downloaded = state
            .storage_client
            .download(
                &dir_meta,
                ORIGINAL_PATH,
                &orig_filename,
                &new_filename,
                &content_type,
                &state.config.upload_dir,
            )
            .await
            .context(StorageSnafu)?;

        create_file_svc(state, &dir, &downloaded).await?
    } else {
        create_remote_file_svc(state, &dir, &upload_claims_copy).await?
    };

    let file = storage_client
        .attach_url(&dir_meta, file)
        .await
        .context(StorageSnafu)?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&file).unwrap(),
    ))
}

pub async fn get_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    Extension(file): Extension<FileDto>,
) -> Result<JsonResponse> {
    let actor = actor.actor.expect("Actor must be present");
    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket.clone(),
        org_id: actor.org_id,
        dir_type: dir.dir_type.clone(),
        dir_name: dir.name.clone(),
    };

    match dir.dir_type {
        DirType::Notes => Ok(JsonResponse::new(serde_json::to_string(&file).unwrap())),
        _ => {
            let storage_client = state.storage_client.clone();
            // Extract dir from the middleware extension
            let file_dto = storage_client
                .attach_url(&dir_meta, file)
                .await
                .context(StorageSnafu)?;

            Ok(JsonResponse::new(serde_json::to_string(&file_dto).unwrap()))
        }
    }
}

pub async fn delete_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    Extension(file): Extension<FileDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let actor = actor.actor.expect("Actor must be present");
    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket.clone(),
        org_id: actor.org_id,
        dir_type: dir.dir_type.clone(),
        dir_name: dir.name.clone(),
    };

    delete_file_svc(state, &dir_meta, &file).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn list_notes_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    query: Query<ListFilesParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Ensure we are at notes dir
    ensure!(
        dir.dir_type == DirType::Notes,
        ForbiddenSnafu {
            msg: "Notes are not allowed on non-notes directories"
        }
    );

    let files = state.db.files.list(&dir, &query).await.context(DbSnafu)?;

    Ok(JsonResponse::new(serde_json::to_string(&files).unwrap()))
}

pub async fn create_note_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<DirDto>,
    payload: CoreResult<Json<CreateNoteDto>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesCreate];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Ensure we are at notes dir
    ensure!(
        dir.dir_type == DirType::Notes,
        ForbiddenSnafu {
            msg: "Notes are not allowed on non-notes directories"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let file = create_note_svc(&state, &dir, &data).await?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&file).unwrap(),
    ))
}

pub async fn get_note_handler(
    State(state): State<AppState>,
    Extension(file): Extension<FileDto>,
) -> Result<JsonResponse> {
    let opt_note = get_note_svc(&state, &file.id).await?;
    let note = opt_note.context(NotFoundSnafu {
        msg: "Note not found",
    })?;

    let note_content: NoteContentDto = note.into();

    Ok(JsonResponse::new(
        serde_json::to_string(&note_content).unwrap(),
    ))
}

pub async fn update_note_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(file): Extension<FileDto>,
    payload: CoreResult<Json<UpdateNoteDto>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let opt_note = get_note_svc(&state, &file.id).await?;
    let _ = opt_note.context(NotFoundSnafu {
        msg: "Note not found",
    })?;

    let updated_content = data.content.clone();

    let updated = update_note_svc(&state, file.id.clone(), data.0).await?;
    let mut note_content: NoteContentDto = updated.into();

    // Replace with plain text
    note_content.content = updated_content;

    Ok(JsonResponse::new(
        serde_json::to_string(&note_content).unwrap(),
    ))
}
