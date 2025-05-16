use axum::{
    Extension,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;

use crate::{
    Error, Result,
    ctx::{Ctx, CtxValue},
    error::ErrorInfo,
    models::Pref,
    run::AppState,
    services::authenticate_token,
    web::{AUTH_TOKEN_COOKIE, handle_error},
};

/// Validates auth token but does not require its validity
pub async fn auth_middleware(
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
    cookies: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    let config = state.config.clone();
    let token = cookies
        .get(AUTH_TOKEN_COOKIE)
        .map(|c| c.value().to_string());

    let full_page = req.headers().get("HX-Request").is_none();

    // Allow ctx to be always present
    let mut ctx: Ctx = Ctx::new(None);

    if let Some(token) = token {
        // Validate token
        let result = authenticate_token(&config.api_url, &token).await;

        let _ = match result {
            Ok(actor) => {
                ctx = Ctx::new(Some(CtxValue::new(token, actor)));
            }
            Err(err) => match err {
                Error::LoginRequired => {
                    // Allow passing through
                    ()
                }
                _ => return handle_error(&state, None, &pref, ErrorInfo::from(&err), full_page),
            },
        };
    }

    req.extensions_mut().insert(ctx);
    next.run(req).await
}

pub async fn require_auth_middleware(
    Extension(ctx): Extension<Ctx>,
    req: Request,
    next: Next,
) -> Result<Response> {
    let full_page = req.headers().get("HX-Request").is_none();

    if ctx.value.is_none() {
        if full_page {
            return Ok(Redirect::to("/login").into_response());
        } else {
            return Err(Error::LoginRequired);
        }
    }

    Ok(next.run(req).await)
}
