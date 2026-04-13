use snafu::{OptionExt, ResultExt, ensure};
use validator::Validate;

use crate::auth::token::decode_auth_token;
use crate::error::{
    DbSnafu, InactiveUserSnafu, InvalidClientSnafu, InvalidPasswordSnafu, UserNotFoundSnafu,
    ValidationSnafu, WhateverSnafu,
};
use crate::{Result, state::AppState};
use memo::validators::flatten_errors;
use password::verify_password;
use yaas::actor::{Actor, ActorPayload, AuthResponse};

pub async fn authenticate_token(state: &AppState, token: &str) -> Result<Actor> {
    let claims = decode_auth_token(token)?;

    // Get from cache first
    if let Some(actor) = state.auth_cache.get(&claims.sub) {
        return Ok(actor);
    }

    let url = format!("{}/oauth/profile", &state.config.auth.api_url);
    let response = state
        .client
        .get(url.as_str())
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to process auth information. Try again later.".to_string(),
        })?;

    match response.status() {
        StatusCode::OK => {
            let actor = response
                .json::<ActorDto>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to parse auth information".to_string(),
                })?;

            // Store to cache
            state.auth_cache.insert(
                claims.sub,
                Actor {
                    actor: Some(actor.clone()),
                },
            );

            Ok(Actor { actor: Some(actor) })
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired),
        _ => {
            info!("Auth API returned status code: {}", response.status());
            Err("Unable to process auth information. Try again later.".into())
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthClaims {
    pub sub: String,
    pub oid: String,
    pub roles: String,
    pub scope: String,
    pub exp: usize,
}

pub fn decode_auth_token(token: &str) -> Result<AuthClaims> {
    let chunks: Vec<&str> = token.split('.').collect();
    if let Some(data_chunk) = chunks.get(1) {
        let decoded = BASE64_URL_SAFE_NO_PAD
            .decode(*data_chunk)
            .context(Base64DecodeSnafu)?;

        let claims: AuthClaims = serde_json::from_slice(&decoded).context(JwtClaimsParseSnafu)?;
        return Ok(claims);
    }

    Err("Invalid auth token.".into())
}
