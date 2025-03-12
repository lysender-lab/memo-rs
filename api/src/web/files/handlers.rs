use axum::{
    Extension,
    extract::{Multipart, Query, State},
    http::StatusCode,
};
use tokio::{fs::File, fs::create_dir_all, io::AsyncWriteExt};

use crate::{
    Error, Result,
    auth::Actor,
    buckets::BucketDto,
    dirs::Dir,
    files::{
        FileDto, FileObject, FilePayload, ImgVersion, ListFilesParams, create_file, delete_file,
        list_files,
    },
    roles::Permission,
    storage::{delete_file_object, format_file, format_files},
    util::slugify_prefixed,
    web::{pagination::Paginated, response::JsonResponse, server::AppState},
};

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
