use reqwest::StatusCode;
use snafu::ResultExt;
use tracing::info;

use crate::{
    Error, Result,
    error::{ErrorMessageDto, HttpClientSnafu, HttpResponseParseSnafu},
    run::AppState,
    services::token::decode_auth_token,
};
use yaas::{
    actor::{Actor, ActorDto},
    oauth::{OauthTokenRequestDto, OauthTokenResponseDto},
};

pub async fn authenticate_token(state: &AppState, token: &str) -> Result<Actor> {
    // Decode token to get user ID (sub claim)
    let claims = decode_auth_token(token)?;

    // Get from cache first
    if let Some(actor) = state.auth_cache.get(&claims.sub) {
        return Ok(actor);
    }

    // Hit the auth server directly
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

pub async fn exchange_code_for_access_token(
    state: &AppState,
    payload: &OauthTokenRequestDto,
) -> Result<OauthTokenResponseDto> {
    let url = format!("{}/oauth/token", &state.config.auth.api_url);

    let response = state
        .client
        .post(url)
        .json(payload)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to exchange token. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_oauth_error(response).await);
    }

    Ok(response
        .json::<OauthTokenResponseDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse oauth information.".to_string(),
        })?)
}

async fn handle_oauth_error(response: reqwest::Response) -> Error {
    let Some(content_type) = response.headers().get("Content-Type") else {
        return Error::Oauth {
            msg: "Unable to identify service response type".to_string(),
        };
    };

    let Ok(content_type) = content_type.to_str() else {
        return Error::Oauth {
            msg: "Unable to identify service response type".to_string(),
        };
    };

    match content_type {
        "application/json" => {
            let Ok(error) = response.json::<ErrorMessageDto>().await else {
                return Error::Oauth {
                    msg: "Unable to parse JSON service error response".to_string(),
                };
            };

            Error::Oauth { msg: error.message }
        }
        "text/plain" | "text/plain; charset=utf-8" => {
            // Probably some default http error
            let text_res = response.text().await;
            Error::Oauth {
                msg: match text_res {
                    Ok(text) => text,
                    Err(_) => "Unable to parse text service error response".to_string(),
                },
            }
        }
        _ => Error::Oauth {
            msg: "Unable to parse service error response".to_string(),
        },
    }
}
