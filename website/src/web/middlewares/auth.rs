use axum::{
    Extension,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;

use crate::{
    Error,
    ctx::Ctx,
    error::ErrorInfo,
    models::Pref,
    run::AppState,
    services::authenticate_token,
    web::{AUTH_TOKEN_COOKIE, handle_error},
};

pub async fn require_auth_middleware(
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

    let Some(token) = token else {
        return Redirect::to("/login").into_response();
    };

    let full_page = req.headers().get("HX-Request").is_none();

    // Validate token
    let result = authenticate_token(&config.api_url, &token).await;

    match result {
        Ok(actor) => {
            let ctx = Ctx::new(token, actor);
            req.extensions_mut().insert(ctx);
        }
        Err(err) => match err {
            Error::LoginRequired => {
                if full_page {
                    return Redirect::to("/login").into_response();
                } else {
                    return handle_error(&state, None, &pref, ErrorInfo::from(&err), full_page);
                }
            }
            _ => return handle_error(&state, None, &pref, ErrorInfo::from(&err), full_page),
        },
    }

    next.run(req).await
}
