use axum::{
    Extension,
    extract::{Json, Multipart, Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use core::result::Result as CoreResult;
use serde::Serialize;
use snafu::{OptionExt, ResultExt, ensure};
use tokio::{fs::File, fs::create_dir_all, io::AsyncWriteExt};

use crate::{
    auth::{
        authenticate,
        user::{
            ChangeCurrentPassword, NewUser, UpdateUserPassword, UpdateUserRoles, UpdateUserStatus,
            change_current_password,
        },
    },
    bucket::{NewBucket, create_bucket, delete_bucket},
    client::{
        ClientDefaultBucket, NewClient, UpdateClient, create_client, delete_client, update_client,
    },
    dir::{Dir, ListDirsParams, NewDir, UpdateDir, delete_dir},
    error::{
        CreateFileSnafu, ErrorResponse, ForbiddenSnafu, JsonRejectionSnafu, MissingUploadFileSnafu,
        Result, UploadDirSnafu, WhateverSnafu,
    },
    file::{FileObject, FilePayload, ListFilesParams, create_file},
    health::{check_liveness, check_readiness},
    state::AppState,
    web::{params::Params, response::JsonResponse},
};
use memo::{
    actor::{Actor, Credentials},
    bucket::BucketDto,
    client::ClientDto,
    file::{FileDto, ImgVersion},
    pagination::Paginated,
    role::Permission,
    user::UserDto,
    utils::slugify_prefixed,
};

#[derive(Serialize)]
pub struct AppMeta {
    pub name: String,
    pub version: String,
}

pub async fn authenticate_handler(
    State(state): State<AppState>,
    payload: CoreResult<Json<Credentials>, JsonRejection>,
) -> Result<JsonResponse> {
    let credentials = payload.context(JsonRejectionSnafu {
        msg: "Invalid credentials payload",
    })?;

    let res = authenticate(&state, &credentials).await?;
    Ok(JsonResponse::new(serde_json::to_string(&res).unwrap()))
}

pub async fn profile_handler(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(
        serde_json::to_string(&actor.user).unwrap(),
    ))
}

pub async fn user_permissions_handler(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    let mut items: Vec<String> = actor.permissions.iter().map(|p| p.to_string()).collect();
    items.sort();
    Ok(JsonResponse::new(serde_json::to_string(&items).unwrap()))
}

pub async fn user_authz_handler(Extension(actor): Extension<Actor>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&actor).unwrap()))
}

pub async fn change_password_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    payload: CoreResult<Json<ChangeCurrentPassword>, JsonRejection>,
) -> Result<JsonResponse> {
    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let _ = change_current_password(&state, &actor.user.id, &data).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
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

pub async fn list_clients_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::ClientsList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let mut client_id: Option<String> = None;
    if !actor.is_system_admin() {
        client_id = Some(actor.client_id.clone());
    }
    let clients = state.db.clients.list(client_id).await?;
    let dtos: Vec<ClientDto> = clients.into_iter().map(|x| x.into()).collect();
    Ok(JsonResponse::new(serde_json::to_string(&dtos).unwrap()))
}

pub async fn create_client_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    payload: CoreResult<Json<NewClient>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::ClientsCreate];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let created = create_client(&state, &data, false).await?;
    let dto: ClientDto = created.into();
    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn get_client_handler(Extension(client): Extension<ClientDto>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&client).unwrap()))
}

pub async fn update_client_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
    payload: CoreResult<Json<UpdateClient>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::ClientsEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    // No changes, just return the client
    if data.name.is_none() && data.default_bucket_id.is_none() && data.status.is_none() {
        return Ok(JsonResponse::new(serde_json::to_string(&client).unwrap()));
    }

    let updated = update_client(&state, client.id.as_str(), &data).await?;
    if !updated {
        // No changes, just return the client
        return Ok(JsonResponse::new(serde_json::to_string(&client).unwrap()));
    }

    let updated_client = state.db.clients.get(client.id.as_str()).await?;
    let updated_client = updated_client.context(WhateverSnafu {
        msg: "Unable to find updated client",
    })?;

    Ok(JsonResponse::new(
        serde_json::to_string(&updated_client).unwrap(),
    ))
}

pub async fn delete_client_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::ClientsDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );
    ensure!(
        !client.admin,
        ForbiddenSnafu {
            msg: "Cannot delete admin client"
        }
    );

    let _ = delete_client(&state, &client.id).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn update_default_bucket_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
    payload: CoreResult<Json<ClientDefaultBucket>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::ClientsEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let data = UpdateClient {
        name: None,
        status: None,
        default_bucket_id: Some(data.default_bucket_id.clone()),
    };

    let updated = update_client(&state, client.id.as_str(), &data).await?;
    if !updated {
        // No changes, just return the client
        return Ok(JsonResponse::new(serde_json::to_string(&client).unwrap()));
    }

    let updated_client = state.db.clients.get(client.id.as_str()).await?;
    let updated_client = updated_client.context(WhateverSnafu {
        msg: "Unable to find updated client",
    })?;

    Ok(JsonResponse::new(
        serde_json::to_string(&updated_client).unwrap(),
    ))
}

pub async fn list_buckets_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::BucketsList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );
    let buckets = state.db.buckets.list(&client.id).await?;
    Ok(JsonResponse::new(serde_json::to_string(&buckets).unwrap()))
}

pub async fn get_bucket_handler(Extension(bucket): Extension<BucketDto>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&bucket).unwrap()))
}

pub async fn delete_bucket_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::BucketsDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let _ = delete_bucket(&state, bucket.id.as_str()).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn create_bucket_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
    payload: CoreResult<Json<NewBucket>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::BucketsCreate];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let bucket = create_bucket(&state, &client.id, &data).await?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&bucket).unwrap(),
    ))
}

pub async fn list_users_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );
    let users = state.db.users.list(&client.id).await?;
    let dto: Vec<UserDto> = users.into_iter().map(|x| x.into()).collect();
    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn create_user_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(client): Extension<ClientDto>,
    payload: CoreResult<Json<NewUser>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersCreate];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let user = state.db.users.create(&client.id, &data, false).await?;
    let dto: UserDto = user.into();

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&dto).unwrap(),
    ))
}

pub async fn get_user_handler(
    Extension(actor): Extension<Actor>,
    Extension(user): Extension<UserDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    Ok(JsonResponse::new(serde_json::to_string(&user).unwrap()))
}

pub async fn update_user_status_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(user): Extension<UserDto>,
    payload: CoreResult<Json<UpdateUserStatus>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Do not allow updating your own user
    ensure!(
        &actor.user.id != &user.id,
        ForbiddenSnafu {
            msg: "Updating your own user account not allowed"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    // Ideally, should not update if status do not change
    let _ = state.db.users.update_status(&user.id, &data).await?;

    // Re-query and show
    let updated_user = state.db.users.get(&user.id).await?.context(WhateverSnafu {
        msg: "Unable to re-query user information.",
    })?;
    let dto: UserDto = updated_user.into();

    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn update_user_roles_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(user): Extension<UserDto>,
    payload: CoreResult<Json<UpdateUserRoles>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Do not allow updating your own user
    ensure!(
        &actor.user.id != &user.id,
        ForbiddenSnafu {
            msg: "Updating your own user account not allowed"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    // Ideally, should not update if roles do not change
    let _ = state.db.users.update_roles(&user.id, &data).await?;

    // Re-query and show
    let updated_user = state.db.users.get(&user.id).await?.context(WhateverSnafu {
        msg: "Unable to re-query user information.",
    })?;
    let dto: UserDto = updated_user.into();

    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn reset_user_password_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(user): Extension<UserDto>,
    payload: CoreResult<Json<UpdateUserPassword>, JsonRejection>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersEdit];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Do not allow updating your own user
    ensure!(
        &actor.user.id != &user.id,
        ForbiddenSnafu {
            msg: "Updating your own user account not allowed"
        }
    );

    let data = payload.context(JsonRejectionSnafu {
        msg: "Invalid request payload",
    })?;

    let _ = state.db.users.update_password(&user.id, &data).await?;

    // Re-query and show
    let updated_user = state.db.users.get(&user.id).await?.context(WhateverSnafu {
        msg: "Unable to re-query user information.",
    })?;
    let dto: UserDto = updated_user.into();

    Ok(JsonResponse::new(serde_json::to_string(&dto).unwrap()))
}

pub async fn delete_user_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(user): Extension<UserDto>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::UsersDelete];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Do not allow deleting your own user account
    ensure!(
        &actor.user.id != &user.id,
        ForbiddenSnafu {
            msg: "Deleting your own user account not allowed"
        }
    );

    let _ = state.db.users.delete(&user.id).await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn list_dirs_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    query: Query<ListDirsParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::DirsList];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let dirs = state.db.dirs.list(bucket.id.as_str(), &query).await?;
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

    let dir = state.db.dirs.create(bucket.id.as_str(), &data).await?;
    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&dir).unwrap(),
    ))
}

pub async fn get_dir_handler(Extension(dir): Extension<Dir>) -> Result<JsonResponse> {
    Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap()))
}

pub async fn update_dir_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir): Extension<Dir>,
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

    let updated = state.db.dirs.update(&dir.id, &data).await?;

    // Either return the updated dir or the original one
    match updated {
        true => get_dir_as_response(&state, &dir.id).await,
        false => Ok(JsonResponse::new(serde_json::to_string(&dir).unwrap())),
    }
}

async fn get_dir_as_response(state: &AppState, id: &str) -> Result<JsonResponse> {
    let res = state.db.dirs.get(id).await?;
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
    let _ = delete_dir(&state, &dir_id).await?;
    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}

pub async fn list_files_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    query: Query<ListFilesParams>,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let files = state.db.files.list(&dir, &query).await?;
    let storage_client = state.storage_client.clone();

    // Generate download urls for each files
    let items: Vec<FileDto> = files.data.into_iter().map(|f| f.into()).collect();

    let items = storage_client
        .format_files(&bucket.name, &dir.name, items)
        .await?;
    let listing = Paginated::new(
        items,
        files.meta.page,
        files.meta.per_page,
        files.meta.total_records,
    );
    Ok(JsonResponse::new(serde_json::to_string(&listing).unwrap()))
}

pub async fn create_file_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    mut multipart: Multipart,
) -> Result<JsonResponse> {
    let permissions = vec![Permission::FilesCreate];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

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

        let _ = create_dir_all(orig_dir.clone())
            .await
            .context(UploadDirSnafu)?;

        // Prepare to save to file
        let file_path = orig_dir.as_path().join(&filename);
        let mut file = File::create(&file_path)
            .await
            .context(CreateFileSnafu { path: file_path })?;

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

    let payload = payload.context(MissingUploadFileSnafu {
        msg: "Missing upload file",
    })?;

    let storage_client = state.storage_client.clone();
    let file = create_file(state, &bucket, &dir, &payload).await?;
    let file_dto: FileDto = file.into();
    let file_dto = storage_client
        .format_file(&bucket.name, &dir.name, file_dto)
        .await?;

    Ok(JsonResponse::with_status(
        StatusCode::CREATED,
        serde_json::to_string(&file_dto).unwrap(),
    ))
}

pub async fn get_file_handler(
    State(state): State<AppState>,
    Extension(bucket): Extension<BucketDto>,
    Extension(dir): Extension<Dir>,
    Extension(file): Extension<FileObject>,
) -> Result<JsonResponse> {
    let storage_client = state.storage_client.clone();
    // Extract dir from the middleware extension
    let file_dto: FileDto = file.clone().into();
    let file_dto = storage_client
        .format_file(&bucket.name, &dir.name, file_dto)
        .await?;
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
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // Delete record
    let _ = state.db.files.delete(&file.id).await?;

    // Delete file(s) from storage
    let storage_client = state.storage_client.clone();
    let dto: FileDto = file.into();
    let _ = storage_client
        .delete_file_object(&bucket.name, &dir.name, &dto)
        .await?;

    Ok(JsonResponse::with_status(
        StatusCode::NO_CONTENT,
        "".to_string(),
    ))
}
