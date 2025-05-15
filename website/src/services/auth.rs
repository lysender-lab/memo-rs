use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;

use crate::{
    Error, Result,
    error::{HttpClientSnafu, HttpResponseParseSnafu},
};
use memo::actor::Actor;

#[derive(Serialize)]
pub struct AuthPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn authenticate(api_url: &str, data: AuthPayload) -> Result<AuthResponse> {
    let mut body = HashMap::new();
    body.insert("username", data.username);
    body.insert("password", data.password);

    let url = format!("{}/auth/token", api_url);
    let response = Client::new()
        .post(url.as_str())
        .json(&body)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to process login information. Try again later.".to_string(),
        })?;

    match response.status() {
        StatusCode::OK => {
            let auth = response
                .json::<AuthResponse>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to parse user information. Try again later.".to_string(),
                })?;
            Ok(auth)
        }
        StatusCode::BAD_REQUEST => Err(Error::LoginFailed),
        StatusCode::UNAUTHORIZED => Err(Error::LoginFailed),
        _ => Err("Unable to process login information. Try again later.".into()),
    }
}

pub async fn authenticate_token(api_url: &str, token: &str) -> Result<Actor> {
    let url = format!("{}/user/authz", api_url);
    let response = Client::new()
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
                .json::<Actor>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to process auth information. Try again later.".to_string(),
                })?;
            Ok(actor)
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired),
        _ => Err("Unable to process auth information. Try again later.".into()),
    }
}
