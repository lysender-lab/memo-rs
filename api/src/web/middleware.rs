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
    auth::authenticate_token,
    error::{
        BadRequestSnafu, ForbiddenSnafu, InsufficientAuthScopeSnafu, InvalidAuthTokenSnafu,
        NotFoundSnafu,
    },
    state::AppState,
    web::params::Params,
};
use memo::{actor::Actor, role::Permission, user::UserDto, utils::valid_id};

use super::params::{ClientParams, UserParams};

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

    // Ensure regular clients can only view their own clients
    if !actor.is_system_admin() {
        ensure!(
            actor.client_id.as_str() == params.client_id.as_str(),
            NotFoundSnafu {
                msg: "Client not found"
            }
        )
    }

    let client = state.db.clients.get(&params.client_id).await?;
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

    let bucket = state.db.buckets.get(&params.bucket_id).await?;
    let bucket = bucket.context(NotFoundSnafu {
        msg: "Bucket not found",
    })?;

    if !actor.is_system_admin() {
        ensure!(
            &bucket.client_id == &actor.client_id,
            NotFoundSnafu {
                msg: "Bucket not found"
            }
        );
    }

    // Forward to the next middleware/handler passing the bucket information
    request.extensions_mut().insert(bucket);
    let response = next.run(request).await;
    Ok(response)
}

pub async fn user_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<UserParams>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let permissions = vec![Permission::UsersList, Permission::UsersView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    ensure!(
        valid_id(&params.user_id),
        BadRequestSnafu {
            msg: "Invalid user id"
        }
    );

    let user = state.db.users.get(&params.user_id).await?;
    let user = user.context(NotFoundSnafu {
        msg: "User not found",
    })?;

    if !actor.is_system_admin() {
        ensure!(
            &user.client_id == &actor.client_id,
            NotFoundSnafu {
                msg: "User not found"
            }
        );
    }

    let user: UserDto = user.into();

    // Forward to the next middleware/handler passing the bucket information
    request.extensions_mut().insert(user);
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
    let dir_res = state.db.dirs.get(&did).await?;

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
    let file_res = state.db.files.get(&fid).await?;
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
