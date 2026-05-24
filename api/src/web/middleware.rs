use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use snafu::{OptionExt, ResultExt, ensure};

use crate::{
    Error, Result,
    error::{
        DbSnafu, ForbiddenSnafu, InsufficientAuthScopeSnafu, InvalidAuthTokenSnafu, NotFoundSnafu,
    },
    oauth::authenticate_token,
    state::AppState,
    web::params::{DirParams, DirTypeParams, FileParams},
};
use memo::{
    dir::{DirDto, DirType},
    file::FileDto,
};
use yaas::{actor::Actor, role::Permission};

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
    ensure!(actor.has_oauth_scope(), InsufficientAuthScopeSnafu);

    Ok(next.run(request).await)
}

/// Ensure that dir_type is valid
pub async fn dir_type_middleware(
    Path(params): Path<DirTypeParams>,
    mut request: Request,
    next: Next,
) -> Result<Response<Body>> {
    let Ok(dir_type) = DirType::try_from(params.dir_type.as_str()) else {
        return Err(Error::BadRequest {
            msg: format!("Invalid dir type: {}", params.dir_type),
        });
    };

    // Forward to the next middleware/handler passing the dir_type information
    request.extensions_mut().insert(dir_type);

    let response = next.run(request).await;
    Ok(response)
}

pub async fn dir_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Extension(dir_type): Extension<DirType>,
    Path(params): Path<DirParams>,
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

    let mut dir_res: Option<DirDto> = state.dir_cache.get(&params.dir_id);

    if dir_res.is_none() {
        // Fetch from database
        dir_res = state.db.dirs.get(&params.dir_id).await.context(DbSnafu)?;

        if let Some(d) = dir_res.clone() {
            // Store to cache if present
            state.dir_cache.insert(params.dir_id.clone(), d);
        }
    }

    let dir = dir_res.context(NotFoundSnafu {
        msg: "Directory not found",
    })?;

    let dto: DirDto = dir;

    // Org must match
    let actor = actor
        .actor
        .expect("Actor must be present in auth middleware");

    ensure!(
        dto.org_id == actor.org_id,
        NotFoundSnafu {
            msg: "Directory not found"
        }
    );

    // Dir type must match
    ensure!(
        dto.dir_type == dir_type,
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
    Path(params): Path<FileParams>,
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

    let mut file_res: Option<FileDto> = state.file_cache.get(&params.file_id);

    if file_res.is_none() {
        // Fetch from database
        file_res = state.db.files.get(&params.file_id).await.context(DbSnafu)?;

        if let Some(f) = file_res.clone() {
            // Store to cache if present
            state.file_cache.insert(params.file_id.clone(), f);
        }
    }

    let file = file_res.context(NotFoundSnafu {
        msg: "File not found",
    })?;

    ensure!(
        file.dir_id == params.dir_id,
        NotFoundSnafu {
            msg: "File not found"
        }
    );

    // Forward to the next middleware/handler passing the file information
    request.extensions_mut().insert(file);
    let response = next.run(request).await;
    Ok(response)
}
