use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};

use crate::{
    auth::{actor::Actor, authenticate_token},
    bucket::get_bucket,
    dir::get_dir,
    file::get_file,
    web::{params::Params, server::AppState},
};
use memo::Error;
use memo::error::{create_json_error_response, to_json_error_response};
use memo::role::Permission;
use memo::utils::valid_id;

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    // Middleware to extract actor information from the request
    // Do not enforce authentication here, just extract the actor information
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let mut actor: Option<Actor> = None;

    if let Some(auth_header) = auth_header {
        // At this point, authentication must be verified
        if !auth_header.starts_with("Bearer ") {
            return to_json_error_response(Error::InvalidAuthToken);
        }
        let token = auth_header.replace("Bearer ", "");

        let res = authenticate_token(&state, &token).await;
        match res {
            Ok(data) => {
                actor = Some(data);
            }
            Err(e) => {
                return to_json_error_response(e);
            }
        }
    }

    if let Some(actor) = actor {
        // Forward to the next middleware/handler passing the actor information
        request.extensions_mut().insert(actor);
    }

    let response = next.run(request).await;
    response
}

pub async fn require_auth_middleware(
    actor: Extension<Actor>,
    request: Request,
    next: Next,
) -> Response<Body> {
    //let Some(actor) = actor else {
    //    return to_error_response(Error::NoAuthToken);
    //};
    if !actor.has_auth_scope() {
        return to_json_error_response(Error::InsufficientAuthScope);
    }

    next.run(request).await
}

pub async fn bucket_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    if !actor.has_files_scope() {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient auth scope".to_string(),
            "Forbidden".to_string(),
        );
    }
    let permissions = vec![Permission::BucketsList, Permission::BucketsView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions".to_string(),
            "Forbidden".to_string(),
        );
    }

    if !valid_id(&params.bucket_id) {
        return create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid bucket id".to_string(),
            "Bad Request".to_string(),
        );
    }

    let bucket = get_bucket(&state.db_pool, &params.bucket_id).await;
    let Ok(bucket) = bucket else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting bucket".to_string(),
            "Internal Server Error".to_string(),
        );
    };

    let Some(bucket) = bucket else {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Bucket not found".to_string(),
            "Not Found".to_string(),
        );
    };

    if &bucket.client_id != &actor.client_id {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Bucket not found".to_string(),
            "Not Found".to_string(),
        );
    }

    // Forward to the next middleware/handler passing the bucket information
    request.extensions_mut().insert(bucket);
    let response = next.run(request).await;
    response
}

pub async fn dir_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    if !actor.has_files_scope() {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient auth scope".to_string(),
            "Forbidden".to_string(),
        );
    }

    let permissions = vec![Permission::DirsList, Permission::DirsView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions".to_string(),
            "Forbidden".to_string(),
        );
    }

    let did = params.dir_id.clone().expect("dir_id is required");
    let query_res = get_dir(&state.db_pool, &did).await;
    let Ok(dir_res) = query_res else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting directory".to_string(),
            "Internal Server Error".to_string(),
        );
    };

    let Some(dir) = dir_res else {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Directory not found".to_string(),
            "Not Found".to_string(),
        );
    };

    if &dir.bucket_id != &params.bucket_id {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Directory not found".to_string(),
            "Not Found".to_string(),
        );
    }

    // Forward to the next middleware/handler passing the directory information
    request.extensions_mut().insert(dir);
    let response = next.run(request).await;
    response
}

pub async fn file_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions".to_string(),
            "Forbidden".to_string(),
        );
    }

    let did = params.dir_id.clone().expect("dir_id is required");
    let fid = params.file_id.clone().expect("file_id is required");
    let query_res = get_file(&state.db_pool, &fid).await;
    let Ok(file_res) = query_res else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting file".to_string(),
            "Internal Server Error".to_string(),
        );
    };

    let Some(file) = file_res else {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "File not found".to_string(),
            "Not Found".to_string(),
        );
    };

    if &file.dir_id != &did {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "File not found".to_string(),
            "Not Found".to_string(),
        );
    }

    // Forward to the next middleware/handler passing the file information
    request.extensions_mut().insert(file);
    let response = next.run(request).await;
    response
}
