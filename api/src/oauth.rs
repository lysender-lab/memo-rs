use snafu::ResultExt;

use crate::dto::{ErrorMessageDto, OauthTokenRequestDto, OauthTokenResponseDto, UserDto};
use crate::error::{HttpClientSnafu, HttpResponseParseSnafu};
use crate::run::AppState;
use crate::{Error, Result};

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

pub async fn oauth_profile(state: &AppState, token: &str) -> Result<UserDto> {
    let url = format!("{}/user", &state.config.auth.api_url);

    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to fetch oauth profile. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_oauth_error(response).await);
    }

    Ok(response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.".to_string(),
        })?)
}

async fn handle_oauth_error(response: reqwest::Response) -> Error {
    let Some(content_type) = response.headers().get("Content-Type") else {
        return Error::Service {
            msg: "Unable to identify service response type".to_string(),
        };
    };

    let Ok(content_type) = content_type.to_str() else {
        return Error::Service {
            msg: "Unable to identify service response type".to_string(),
        };
    };

    match content_type {
        "application/json" => {
            let Ok(error) = response.json::<ErrorMessageDto>().await else {
                return Error::Service {
                    msg: "Unable to parse JSON service error response".to_string(),
                };
            };

            Error::Oauth { msg: error.message }
        }
        "text/plain" | "text/plain; charset=utf-8" => {
            // Probably some default http error
            let text_res = response.text().await;
            Error::Service {
                msg: match text_res {
                    Ok(text) => text,
                    Err(_) => "Unable to parse text service error response".to_string(),
                },
            }
        }
        _ => Error::Service {
            msg: "Unable to parse service error response".to_string(),
        },
    }
}
