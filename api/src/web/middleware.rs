use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use snafu::{OptionExt, ResultExt, ensure};
use tracing::info;
use yaas::{actor::Actor, role::Permission};

use crate::{
    Error, Result,
    error::{
        BadRequestSnafu, DbSnafu, ForbiddenSnafu, InsufficientAuthScopeSnafu,
        InvalidAuthTokenSnafu, NotFoundSnafu,
    },
    oauth::authenticate_token,
    state::AppState,
    web::params::Params,
};
use memo::{dir::DirDto, utils::valid_id};

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
    let mut actor: Actor = Actor::default();

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

pub async fn bucket_middleware(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let permissions = vec![Permission::BucketsView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    // ensure!(
    //     valid_id(&params.bucket_id),
    //     BadRequestSnafu {
    //         msg: "Invalid bucket id"
    //     }
    // );

    info!("Fetching bucket with id: {}", params.bucket_id);

    let bucket = state
        .db
        .buckets
        .get(&params.bucket_id)
        .await
        .context(DbSnafu)?;

    let bucket = bucket.context(NotFoundSnafu {
        msg: "Bucket not found",
    })?;

    let Some(actor) = actor.actor.as_ref() else {
        return Err(Error::InvalidAuthToken);
    };

    ensure!(
        bucket.id == actor.org_id,
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
    let permissions = vec![Permission::DirsList, Permission::DirsView];
    ensure!(
        actor.has_permissions(&permissions),
        ForbiddenSnafu {
            msg: "Insufficient permissions"
        }
    );

    let did = params.dir_id.clone().expect("dir_id is required");
    let dir_res = state.db.dirs.get(&did).await.context(DbSnafu)?;

    let dir = dir_res.context(NotFoundSnafu {
        msg: "Directory not found",
    })?;

    let dto: DirDto = dir;

    ensure!(
        dto.bucket_id == params.bucket_id,
        NotFoundSnafu {
            msg: "Directory not found"
        }
    );

    // Forward to the next middleware/handler passing the directory information
    request.extensions_mut().insert(dto);
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
    let file_res = state.db.files.get(&fid).await.context(DbSnafu)?;
    let file = file_res.context(NotFoundSnafu {
        msg: "File not found",
    })?;

    ensure!(
        file.dir_id == did,
        NotFoundSnafu {
            msg: "File not found"
        }
    );

    // Forward to the next middleware/handler passing the file information
    request.extensions_mut().insert(file);
    let response = next.run(request).await;
    Ok(response)
}
