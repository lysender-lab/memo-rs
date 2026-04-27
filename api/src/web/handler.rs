use core::result::Result as CoreResult;

use axum::{
    Extension,
    extract::{Json, Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use snafu::{OptionExt, ResultExt, ensure};

use crate::{
    dir::{create_dir, delete_dir, update_dir},
    error::{
        DbSnafu, ErrorResponse, ForbiddenSnafu, JsonRejectionSnafu, Result, StorageSnafu,
        WhateverSnafu,
    },
    file::{create_file, generate_upload_url},
    health::{check_liveness, check_readiness},
    state::AppState,
    token::verify_upload_token,
    web::{
        params::{DirTypeParams, Params},
        response::JsonResponse,
    },
};
use db::dir::{ListDirsParams, NewDir, UpdateDir};
use db::file::ListFilesParams;
use memo::{
    bucket::BucketDto,
    dir::DirDto,
    file::{FileDto, ORIGINAL_PATH, RemoteUploadDto, SignedRemoteUploadDto},
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
    Path(params): Path<DirTypeParams>,
    query: Query<ListDirsParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let dirs = state
        .db
        .dirs
        .list(bucket.id.as_str(), &query)
        .await
        .context(DbSnafu)?;

    Ok(JsonResponse::new(serde_json::to_string(&dirs).unwrap()))
}

pub async fn create_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
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

    let dir = create_dir(&state, &bucket.id, &data).await?;

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

    let updated = update_dir(&state, &dir.id, &data).await?;

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
    Path(params): Path<Params>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let dir_id = params.dir_id.clone().expect("dir_id is required");
    delete_dir(&state, &dir_id).await?;
    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn list_files_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
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

    let files = state.db.files.list(&dir, &query).await.context(DbSnafu)?;
    let storage_client = state.storage_client.clone();

    // Generate download urls for each files
    let items = storage_client
        .attach_urls(&bucket.name, &dir.name, files.data)
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
    Extension(bucket): Extension<BucketDto>,
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

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let dto = generate_upload_url(state, &bucket, &dir, &data.0).await?;

    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn create_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
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

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    // Validate token
    let upload_claims = verify_upload_token(&data.token, &state.config.jwt_secret)?;
    let orig_filename = upload_claims.orig_filename;
    let new_filename = upload_claims.new_filename;

    // Download file locally
    let downloaded = state
        .storage_client
        .download(
            &bucket.name,
            &dir.name,
            ORIGINAL_PATH,
            &orig_filename,
            &new_filename,
            &state.config.upload_dir,
        )
        .await
        .context(StorageSnafu)?;

    let storage_client = state.storage_client.clone();
    let file = create_file(state, &bucket, &dir, &downloaded).await?;
    let file_dto: FileDto = file;
    let file_dto = storage_client
        .attach_url(&bucket.name, &dir.name, file_dto)
        .await
        .context(StorageSnafu)?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&file_dto).unwrap(),
    ))
}

pub async fn get_file_handler(
    State(state): State<AppState>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<DirDto>,
    Extension(file): Extension<FileDto>,
) -> Result<JsonResponse> {
    let storage_client = state.storage_client.clone();
    // Extract dir from the middleware extension
    let file_dto = storage_client
        .attach_url(&bucket.name, &dir.name, file)
        .await
        .context(StorageSnafu)?;
    Ok(JsonResponse::new(serde_json::to_string(&file_dto).unwrap()))
}

pub async fn delete_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
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

    // Delete record
    state.db.files.delete(&file.id).await.context(DbSnafu)?;
    state.file_cache.remove(&file.id);

    // Delete file(s) from storage
    let storage_client = state.storage_client.clone();
    storage_client
        .delete(&bucket.name, &dir.name, &file)
        .await
        .context(StorageSnafu)?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}
