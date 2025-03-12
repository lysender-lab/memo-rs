use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::{
    auth::Actor,
    files::get_file,
    roles::Permission,
    web::{params::Params, response::create_error_response, server::AppState},
};

pub async fn file_middleware(
    state: State<AppState>,
    Extension(actor): Extension<Actor>,
    Path(params): Path<Params>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    let permissions = vec![Permission::FilesList, Permission::FilesView];
    if !actor.has_permissions(&permissions) {
        return create_error_response(
            StatusCode::FORBIDDEN,
            "Insufficient permissions".to_string(),
            "Forbidden".to_string(),
        );
    }

    let did = params.dir_id.clone().expect("dir_id is required");
    let fid = params.file_id.clone().expect("file_id is required");
    let query_res = get_file(&state.db_pool, &fid).await;
    let Ok(file_res) = query_res else {
        return create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error getting file".to_string(),
            "Internal Server Error".to_string(),
        );
    };

    let Some(file) = file_res else {
        return create_error_response(
            StatusCode::NOT_FOUND,
            "File not found".to_string(),
            "Not Found".to_string(),
        );
    };

    if &file.dir_id != &did {
        return create_error_response(
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
