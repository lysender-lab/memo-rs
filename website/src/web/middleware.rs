use axum::{
    Extension,
    extract::{Path, Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use urlencoding::encode;

use crate::{
    Error, Result,
    ctx::Ctx,
    error::ErrorInfo,
    models::{DirParams, DirTypeParams, FileParams, Pref},
    run::AppState,
    services::{dirs::get_dir_svc, files::get_photo_svc, oauth::authenticate_token},
    web::{Action, Resource, enforce_policy, handle_error},
};
use memo::dir::{DirDto, DirType};
use yaas::actor::Actor;

use super::{AUTH_TOKEN_COOKIE, THEME_COOKIE};

/// Validates auth token but does not require its validity
pub async fn auth_middleware(
    Extension(pref): Extension<Pref>,
    State(state): State<AppState>,
    cookies: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    let token = cookies
        .get(AUTH_TOKEN_COOKIE)
        .map(|c| c.value().to_string());

    let full_page = req.headers().get("HX-Request").is_none();

    // Allow ctx to be always present
    let mut ctx: Ctx = Ctx::default();

    if let Some(token) = token {
        // Validate token
        let result = authenticate_token(&state, &token).await;

        match result {
            Ok(actor) => {
                ctx = Ctx::new(Some(token), actor);
            }
            Err(err) => match err {
                Error::LoginRequired => {
                    // Allow passing through
                }
                Error::InvalidAuthToken => {
                    // Allow passing through
                }
                Error::JwtClaimsParse { .. } => {
                    // Allow passing through
                }
                _ => {
                    let actor = Actor::default();
                    return handle_error(&state, &actor, &pref, ErrorInfo::from(&err), full_page);
                }
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

    if !ctx.is_authenticated() {
        if full_page {
            return Ok(Redirect::to("/login").into_response());
        } else {
            return Err(Error::LoginRequired);
        }
    }

    Ok(next.run(req).await)
}

pub fn build_oauth_authorize_url(state: &AppState) -> String {
    let callback_url = format!("{}/auth/callback", &state.config.server.public_url);
    let scope = encode("oauth");
    let oauth_state = Utc::now().timestamp_millis();

    format!(
        "{}/oauth/authorize?client_id={}&scope={}&state={}&redirect_uri={}",
        state.config.auth.auth_url, state.config.auth.client_id, scope, oauth_state, callback_url
    )
}

/// Ensure that dir_type is valid
pub async fn dir_type_middleware(
    Path(params): Path<DirTypeParams>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    let Ok(dir_type) = DirType::try_from(params.dir_type.as_str()) else {
        return Err(Error::BadRequest {
            msg: format!("Invalid dir type: {}", params.dir_type),
        });
    };

    if dir_type != DirType::Photos && dir_type != DirType::Documents {
        return Err(Error::BadRequest {
            msg: format!("Unsupported dir type: {}", params.dir_type),
        });
    }

    // Forward to the next middleware/handler passing the dir_type information
    request.extensions_mut().insert(dir_type);

    let response = next.run(request).await;
    Ok(response)
}

pub async fn dir_middleware(
    Extension(ctx): Extension<Ctx>,
    Extension(dir_type): Extension<DirType>,
    State(state): State<AppState>,
    Path(params): Path<DirParams>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Album, Action::Read)?;

    let token = ctx.token().expect("token is required");

    let mut dir_res = state.dir_cache.get(&params.dir_id);
    if dir_res.is_none() {
        // Fetch from api
        let dir = get_dir_svc(&state, token, &dir_type, &params.dir_id).await?;

        state.dir_cache.insert(params.dir_id.clone(), dir.clone());

        dir_res = Some(dir);
    }

    let dir = dir_res.expect("Dir is required");

    req.extensions_mut().insert(dir);
    Ok(next.run(req).await)
}

pub async fn file_middleware(
    State(state): State<AppState>,
    Extension(ctx): Extension<Ctx>,
    Extension(dir): Extension<DirDto>,
    Path(params): Path<FileParams>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let actor = ctx.actor();
    enforce_policy(actor, Resource::Photo, Action::Read)?;

    let token = ctx.token().expect("token is required");

    let mut photo_res = state.file_cache.get(&params.file_id);

    if photo_res.is_none() {
        // Fetch from api
        let photo = get_photo_svc(&state, token, &dir.dir_type, &dir.id, &params.file_id).await?;

        state
            .file_cache
            .insert(params.file_id.clone(), photo.clone());

        photo_res = Some(photo);
    }

    let photo = photo_res.expect("photo should be present at this point");

    req.extensions_mut().insert(photo);
    Ok(next.run(req).await)
}

pub async fn pref_middleware(cookies: CookieJar, mut req: Request, next: Next) -> Response {
    let mut pref = Pref::new();
    let theme = cookies.get(THEME_COOKIE).map(|c| c.value().to_string());

    if let Some(theme) = theme {
        let t = theme.as_str();
        if t == "dark" || t == "light" {
            pref.theme = theme;
        }
    }

    req.extensions_mut().insert(pref);
    next.run(req).await
}
