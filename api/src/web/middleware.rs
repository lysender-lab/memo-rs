use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use snafu::{OptionExt, ensure};

use crate::{
    Result,
    auth::{actor::Actor, authenticate_token},
    bucket::get_bucket,
    client::get_client,
    dir::get_dir,
    error::{
        BadRequestSnafu, ForbiddenSnafu, InsufficientAuthScopeSnafu, InvalidAuthTokenSnafu,
        NotFoundSnafu,
    },
    file::get_file,
    web::{params::Params, server::AppState},
};
use memo::{role::Permission, utils::valid_id};

use super::params::ClientParams;

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
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
        ensure!(auth_header.starts_with("Bearer "), InvalidAuthTokenSnafu);
        let token = auth_header.replace("Bearer ", "");

        actor = authenticate_token(&state, &token).await?;
    }

    // Forward to the next middleware/handler passing the actor information
    request.extensions_mut().insert(actor);

    let response = next.run(request).await;
    Ok(response)
}

pub async fn require_auth_middleware(
    actor: Extension<Actor>,
    request: Request,
    next: Next,
) -> Result<Response<Body>> {
    ensure!(actor.has_auth_scope(), InsufficientAuthScopeSnafu);

    Ok(next.run(request).await)
}

pub async fn clients_admin_middleware(
    actor: Extension<Actor>,
    request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let permissions = vec![
        Permission::ClientsList,
        Permission::ClientsView,
        Permission::ClientsManage,
    ];

    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    Ok(next.run(request).await)
}

pub async fn client_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<ClientParams>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let permissions = vec![Permission::ClientsView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    ensure!(
        valid_id(&params.client_id),
        BadRequestSnafu {
            msg: "Invalid client id"
        }
    );

    let client = get_client(&state.db_pool, &params.client_id).await?;
    let client = client.context(NotFoundSnafu {
        msg: "Client not found",
    })?;

    // Forward to the next middleware/handler passing the client information
    request.extensions_mut().insert(client);
    let response = next.run(request).await;
    Ok(response)
}

pub async fn bucket_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    ensure!(
        actor.has_files_scope(),
        ForbiddenSnafu {
            msg: "Insufficient auth scope"
        }
    );

    let permissions = vec![Permission::BucketsList, Permission::BucketsView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    ensure!(
        valid_id(&params.bucket_id),
        BadRequestSnafu {
            msg: "Invalid bucket id"
        }
    );

    let bucket = get_bucket(&state.db_pool, &params.bucket_id).await?;
    let bucket = bucket.context(NotFoundSnafu {
        msg: "Bucket not found",
    })?;

    ensure!(
        &bucket.client_id == &actor.client_id,
        NotFoundSnafu {
            msg: "Bucket not found"
        }
    );

    // Forward to the next middleware/handler passing the bucket information
    request.extensions_mut().insert(bucket);
    let response = next.run(request).await;
    Ok(response)
}

pub async fn dir_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    ensure!(
        actor.has_files_scope(),
        ForbiddenSnafu {
            msg: "Insufficient auth scope"
        }
    );

    let permissions = vec![Permission::DirsList, Permission::DirsView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let did = params.dir_id.clone().expect("dir_id is required");
    let dir_res = get_dir(&state.db_pool, &did).await?;

    let dir = dir_res.context(NotFoundSnafu {
        msg: "Directory not found",
    })?;

    ensure!(
        &dir.bucket_id == &params.bucket_id,
        NotFoundSnafu {
            msg: "Directory not found"
        }
    );

    // Forward to the next middleware/handler passing the directory information
    request.extensions_mut().insert(dir);
    let response = next.run(request).await;
    Ok(response)
}

pub async fn file_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let did = params.dir_id.clone().expect("dir_id is required");
    let fid = params.file_id.clone().expect("file_id is required");
    let file_res = get_file(&state.db_pool, &fid).await?;
    let file = file_res.context(NotFoundSnafu {
        msg: "File not found",
    })?;

    ensure!(
        &file.dir_id == &did,
        NotFoundSnafu {
            msg: "File not found"
        }
    );

    // Forward to the next middleware/handler passing the file information
    request.extensions_mut().insert(file);
    let response = next.run(request).await;
    Ok(response)
}
