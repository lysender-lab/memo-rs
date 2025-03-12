use axum::{
    Extension,
    body::Body,
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::{
    Error,
    auth::{Actor, authenticate_token},
    web::{response::to_error_response, server::AppState},
};

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
            return to_error_response(Error::InvalidAuthToken);
        }
        let token = auth_header.replace("Bearer ", "");

        let res = authenticate_token(&state, &token).await;
        match res {
            Ok(data) => {
                actor = Some(data);
            }
            Err(e) => {
                return to_error_response(e);
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
        return to_error_response(Error::InsufficientAuthScope);
    }

    next.run(request).await
}
