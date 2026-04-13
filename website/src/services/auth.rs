use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use tracing::info;

use crate::{
    Error, Result, error::HttpResponseParseSnafu, run::AppState, services::token::decode_auth_token,
};
use yaas::actor::{Actor, ActorDto};

#[derive(Serialize)]
pub struct AuthPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn authenticate_token(state: &AppState, token: &str) -> Result<Actor> {
    // Decode token to get user ID (sub claim)
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
        StatusCode::UNAUTHORIZED => Err(Error::InvalidAuthToken),
        _ => {
            info!("Auth API returned status code: {}", response.status());
            Err("Unable to process auth information. Try again later.".into())
        }
    }
}
