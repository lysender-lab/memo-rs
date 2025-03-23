use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};

use crate::{
    Error,
    auth::{actor::Actor, authenticate_token},
    bucket::get_bucket,
    client::get_client,
    dir::get_dir,
    error::{create_json_error_response, to_json_error_response},
    file::get_file,
    web::{params::Params, server::AppState},
};
use memo::{role::Permission, utils::valid_id};

use super::params::ClientParams;

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

    // Start with an empty actor
    let mut actor: Actor = Actor::empty();

    if let Some(auth_header) = auth_header {
        // At this point, authentication must be verified
        if !auth_header.starts_with("Bearer ") {
            return to_json_error_response(Error::InvalidAuthToken);
        }
        let token = auth_header.replace("Bearer ", "");

        let res = authenticate_token(&state, &token).await;
        match res {
            Ok(data) => {
                actor = data;
            }
            Err(e) => {
                return to_json_error_response(e);
            }
        }
    }

    // Forward to the next middleware/handler passing the actor information
    request.extensions_mut().insert(actor);

    let response = next.run(request).await;
    response
}

pub async fn require_auth_middleware(
    actor: Extension<Actor>,
    request: Request,
    next: Next,
) -> Response<Body> {
    if !actor.has_auth_scope() {
        return to_json_error_response(Error::InsufficientAuthScope);
    }

    next.run(request).await
}

pub async fn clients_admin_middleware(
    actor: Extension<Actor>,
    request: Request,
    next: Next,
) -> Response<Body> {
    let permissions = vec![
        Permission::ClientsList,
        Permission::ClientsView,
        Permission::ClientsManage,
    ];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions",
            "Forbidden",
        );
    }

    next.run(request).await
}

pub async fn client_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<ClientParams>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    let permissions = vec![Permission::ClientsView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions",
            "Forbidden",
        );
    }

    if !valid_id(&params.client_id) {
        return create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid client id",
            "Bad Request",
        );
    }

    let client = get_client(&state.db_pool, &params.client_id).await;
    let Ok(client) = client else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting client",
            "Internal Server Error",
        );
    };

    let Some(client) = client else {
        return create_json_error_response(StatusCode::NOT_FOUND, "Client not found", "Not Found");
    };

    // Forward to the next middleware/handler passing the client information
    request.extensions_mut().insert(client);
    let response = next.run(request).await;
    response
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
            "Insufficient auth scope",
            "Forbidden",
        );
    }
    let permissions = vec![Permission::BucketsList, Permission::BucketsView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions",
            "Forbidden",
        );
    }

    if !valid_id(&params.bucket_id) {
        return create_json_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid bucket id",
            "Bad Request",
        );
    }

    let bucket = get_bucket(&state.db_pool, &params.bucket_id).await;
    let Ok(bucket) = bucket else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting bucket",
            "Internal Server Error",
        );
    };

    let Some(bucket) = bucket else {
        return create_json_error_response(StatusCode::NOT_FOUND, "Bucket not found", "Not Found");
    };

    if &bucket.client_id != &actor.client_id {
        return create_json_error_response(StatusCode::NOT_FOUND, "Bucket not found", "Not Found");
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
            "Insufficient auth scope",
            "Forbidden",
        );
    }

    let permissions = vec![Permission::DirsList, Permission::DirsView];
    if !actor.has_permissions(&permissions) {
        return create_json_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions",
            "Forbidden",
        );
    }

    let did = params.dir_id.clone().expect("dir_id is required");
    let query_res = get_dir(&state.db_pool, &did).await;
    let Ok(dir_res) = query_res else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting directory",
            "Internal Server Error",
        );
    };

    let Some(dir) = dir_res else {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Directory not found",
            "Not Found",
        );
    };

    if &dir.bucket_id != &params.bucket_id {
        return create_json_error_response(
            StatusCode::NOT_FOUND,
            "Directory not found",
            "Not Found",
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
            "Insufficient permissions",
            "Forbidden",
        );
    }

    let did = params.dir_id.clone().expect("dir_id is required");
    let fid = params.file_id.clone().expect("file_id is required");
    let query_res = get_file(&state.db_pool, &fid).await;
    let Ok(file_res) = query_res else {
        return create_json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting file",
            "Internal Server Error",
        );
    };

    let Some(file) = file_res else {
        return create_json_error_response(StatusCode::NOT_FOUND, "File not found", "Not Found");
    };

    if &file.dir_id != &did {
        return create_json_error_response(StatusCode::NOT_FOUND, "File not found", "Not Found");
    }

    // Forward to the next middleware/handler passing the file information
    request.extensions_mut().insert(file);
    let response = next.run(request).await;
    response
}
