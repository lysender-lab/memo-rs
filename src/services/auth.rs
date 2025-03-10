use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

use crate::{Error, Result, models::Actor};

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

    let url = format!("{}/v1/auth/token", api_url);
    let result = Client::new().post(url.as_str()).json(&body).send().await;
    let Ok(response) = result else {
        return Err("Unable to process login information. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => match response.json::<AuthResponse>().await {
            Ok(auth) => Ok(auth),
            Err(err) => {
                error!("Error: {}", err);
                Err(Error::JsonParseError(
                    "Unable to parse user information. Try again later.".to_string(),
                ))
            }
        },
        StatusCode::BAD_REQUEST => Err(Error::LoginFailed(
            "Invalid username or password".to_string(),
        )),
        StatusCode::UNAUTHORIZED => Err(Error::LoginFailed(
            "Invalid username or password".to_string(),
        )),
        _ => Err("Unable to process login information. Try again later.".into()),
    }
}

pub async fn authenticate_token(api_url: &str, token: &str) -> Result<Actor> {
    let url = format!("{}/v1/user/authz", api_url);
    let result = Client::new()
        .get(url.as_str())
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to process auth information. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => match response.json::<Actor>().await {
            Ok(actor) => Ok(actor),
            Err(_) => Err(Error::JsonParseError(
                "Unable to process auth information. Try again later.".to_string(),
            )),
        },
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login to continue.".to_string())),
        _ => Err("Unable to process auth information. Try again later.".into()),
    }
}
