use axum::{
    Extension,
    extract::{Json, Path, Query, State},
    http::StatusCode,
};

use crate::{
    Error, Result,
    auth::Actor,
    dirs::{
        Dir, ListDirsParams, NewDir, UpdateDir, create_dir, delete_dir, get_dir, list_dirs,
        update_dir,
    },
    roles::Permission,
    web::{params::Params, response::JsonResponse, server::AppState},
};

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
