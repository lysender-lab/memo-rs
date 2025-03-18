use axum::{
    Extension,
    extract::{Json, Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use tokio::{fs::File, fs::create_dir_all, io::AsyncWriteExt};

use crate::{
    auth::{
        actor::{Actor, Credentials},
        authenticate,
    },
    bucket::list_buckets,
    client::list_clients,
    dir::{
        Dir, ListDirsParams, NewDir, UpdateDir, create_dir, delete_dir, get_dir, list_dirs,
        update_dir,
    },
    file::{FileObject, FilePayload, ListFilesParams, create_file, delete_file, list_files},
    health::{check_liveness, check_readiness},
    storage::{delete_file_object, format_file, format_files},
    web::{params::Params, response::JsonResponse, server::AppState},
};
use memo::{
    Error, Result,
    dto::{
        bucket::BucketDto,
        client::ClientDto,
        file::{FileDto, ImgVersion},
        pagination::Paginated,
    },
    error::ErrorResponse,
    role::Permission,
    utils::slugify_prefixed,
};

#[derive(Serialize)]
pub struct AppMeta {
    pub name: String,
    pub version: String,
}

#[axum::debug_handler]
pub async fn authenticate_handler(
    State(state): State<AppState>,
    payload: Json<Credentials>,
) -> Result<JsonResponse> {
    //let Some(credentials) = payload else {
    //    return Err(Error::BadRequest("Invalid credentials payload".into()));
    //};

    let res = authenticate(&state, &payload).await?;
    Ok(JsonResponse::new(serde_json::to_string(&res).unwrap()))
}

pub async fn profile_handler(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(
        serde_json::to_string(&actor.user).unwrap(),
    ))
}

pub async fn user_permissions(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    let mut items: Vec<String> = actor.permissions.iter().map(|p| p.to_string()).collect();
    items.sort();
    Ok(JsonResponse::new(serde_json::to_string(&items).unwrap()))
}

pub async fn user_authz(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&actor).unwrap()))
}

pub async fn home_handler() -> impl IntoResponse {
    Json(AppMeta {
        name: "files-rs".to_string(),
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
    let health = check_readiness(&state.config, &state.db_pool).await?;
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

pub async fn list_clients_handler(State(state): State<AppState>) -> Result<JsonResponse> {
    let clients = list_clients(&state.db_pool).await?;
    Ok(JsonResponse::new(serde_json::to_string(&clients).unwrap()))
}

pub async fn list_buckets_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
) -> Result<JsonResponse> {
    let buckets = list_buckets(&state.db_pool, &actor.client_id).await?;
    Ok(JsonResponse::new(serde_json::to_string(&buckets).unwrap()))
}

pub async fn get_client_handler(Extension(client): Extension<ClientDto>) -> Result<JsonResponse> {
    // Extract bucket from the middleware extension
    Ok(JsonResponse::new(serde_json::to_string(&client).unwrap()))
}

pub async fn get_bucket_handler(Extension(bucket): Extension<BucketDto>) -> Result<JsonResponse> {
    // Extract bucket from the middleware extension
    Ok(JsonResponse::new(serde_json::to_string(&bucket).unwrap()))
}

#[axum::debug_handler]
pub async fn list_dirs_handler(
    State(state): State<AppState>,
    Path(bucket_id): Path<String>,
    query: Query<ListDirsParams>,
) -> Result<JsonResponse> {
    //let Some(params) = query else {
    //    return Err(Error::BadRequest("Invalid query parameters".to_string()));
    //};
    let dirs = list_dirs(&state.db_pool, &bucket_id, &query).await?;
    Ok(JsonResponse::new(serde_json::to_string(&dirs).unwrap()))
}

#[axum::debug_handler]
pub async fn create_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(bucket_id): Path<String>,
    payload: Json<NewDir>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsCreate];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    //let Some(data) = payload else {
    //    return Err(Error::BadRequest("Invalid request payload".to_string()));
    //};
    let dir = create_dir(&state.db_pool, &bucket_id, &payload).await?;
    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&dir).unwrap(),
    ))
}

pub async fn get_dir_handler(Extension(dir): Extension<Dir>) -> Result<JsonResponse> {
    // Extract dir from the middleware extension
    Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap()))
}

#[axum::debug_handler]
pub async fn update_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<Dir>,
    Path(params): Path<Params>,
    payload: Json<UpdateDir>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsEdit];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    let dir_id = params.dir_id.clone().expect("dir_id is required");
    //let Some(data) = payload else {
    //    return Err(Error::BadRequest("Invalid request payload".to_string()));
    //};

    let updated = update_dir(&state.db_pool, &dir_id, &payload).await?;

    // Either return the updated dir or the original one
    match updated {
        true => get_dir_as_response(&state, &dir_id).await,
        false => Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap())),
    }
}

async fn get_dir_as_response(state: &AppState, id: &str) -> Result<JsonResponse> {
    let res = get_dir(&state.db_pool, id).await;
    let Ok(dir_res) = res else {
        return Err("Error getting directory".into());
    };

    let Some(dir) = dir_res else {
        return Err("Error getting directory this time".into());
    };

    Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap()))
}

pub async fn delete_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsDelete];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    let dir_id = params.dir_id.clone().expect("dir_id is required");
    let _ = delete_dir(&state.db_pool, &dir_id).await?;
    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

#[axum::debug_handler]
pub async fn list_files_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    query: Query<ListFilesParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    //let Some(params) = query else {
    //    return Err(Error::BadRequest("Invalid query parameters".to_string()));
    //};
    let files = list_files(&state.db_pool, &dir, &query).await?;
    let storage_client = state.storage_client;

    // Generate download urls for each files
    let items: Vec<FileDto> = files.data.into_iter().map(|f| f.into()).collect();
    let items = format_files(&storage_client, &bucket.name, &dir.name, items).await?;
    let listing = Paginated::new(
        items,
        files.meta.page,
        files.meta.per_page,
        files.meta.total_records,
    );
    Ok(JsonResponse::new(serde_json::to_string(&listing).unwrap()))
}

#[axum::debug_handler]
pub async fn create_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    mut multipart: Multipart,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesCreate];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    let mut payload: Option<FilePayload> = None;

    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        if name != "file" {
            continue;
        }

        let original_filename = field.file_name().unwrap().to_string();

        // Low chance of collision but higher than the full uuid v7 string
        // Prefer a shorter filename for better readability
        let filename = slugify_prefixed(&original_filename);

        // Ensure upload dir exists
        let orig_dir = state
            .config
            .upload_dir
            .clone()
            .join(ImgVersion::Original.to_string());
        let dir_res = create_dir_all(orig_dir.clone()).await;
        if let Err(_) = dir_res {
            return Err("Unable to create upload dir".into());
        }

        // Prepare to save to file
        let file_path = orig_dir.as_path().join(&filename);
        let Ok(mut file) = File::create(&file_path).await else {
            return Err("Unable to create file".into());
        };

        // Stream contents to file
        let mut size: usize = 0;
        while let Some(chunk) = field.chunk().await.unwrap() {
            size += chunk.len();
            file.write_all(&chunk).await.unwrap();
        }

        payload = Some({
            FilePayload {
                upload_dir: state.config.upload_dir.clone(),
                name: original_filename,
                filename: filename.clone(),
                path: orig_dir.clone().join(&filename),
                size: size as i64,
            }
        })
    }

    let Some(payload) = payload else {
        return Err(Error::MissingUploadFile("Missing upload file".to_string()));
    };

    let db_pool = state.db_pool.clone();
    let storage_client = state.storage_client;
    let res = create_file(&db_pool, &storage_client, &bucket, &dir, &payload).await;
    match res {
        Ok(file) => {
            let file_dto: FileDto = file.into();
            let file_dto = format_file(&storage_client, &bucket.name, &dir.name, file_dto).await?;
            Ok(JsonResponse::with_status(
                StatusCode::CREATED,
                serde_json::to_string(&file_dto).unwrap(),
            ))
        }
        Err(e) => Err(e),
    }
}

pub async fn get_file_handler(
    State(state): State<AppState>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    Extension(file): Extension<FileObject>,
) -> Result<JsonResponse> {
    let storage_client = state.storage_client;
    // Extract dir from the middleware extension
    let file_dto: FileDto = file.clone().into();
    let file_dto = format_file(&storage_client, &bucket.name, &dir.name, file_dto).await?;
    Ok(JsonResponse::new(serde_json::to_string(&file_dto).unwrap()))
}

pub async fn delete_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    Extension(file): Extension<FileObject>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesDelete];
    if !actor.has_permissions(&permissions) {
        return Err(Error::Forbidden("Insufficient permissions".to_string()));
    }

    // Delete record
    let db_pool = state.db_pool.clone();
    let _ = delete_file(&db_pool, &file.id).await?;

    // Delete file(s) from storage
    let storage_client = state.storage_client;
    let dto: FileDto = file.into();
    let _ = delete_file_object(&storage_client, &bucket.name, &dir.name, &dto).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}
