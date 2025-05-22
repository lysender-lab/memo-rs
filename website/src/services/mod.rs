pub mod auth;
pub mod buckets;
pub mod captcha;
pub mod clients;
pub mod dirs;
pub mod files;
pub mod token;
pub mod users;

use reqwest::StatusCode;
use snafu::ResultExt;

use crate::{
    Error, Result,
    error::{ErrorResponse, HttpResponseParseSnafu},
};

pub async fn handle_response_error(
    response: reqwest::Response,
    resource: &str,
    not_found: Error,
) -> Error {
    // Assumes that ok responses are already handled
    match response.status() {
        StatusCode::BAD_REQUEST => {
            let message_res = parse_response_error(response).await;
            match message_res {
                Ok(msg) => Error::BadRequest { msg },
                Err(_) => Error::BadRequest {
                    msg: "Bad Request.".to_string(),
                },
            }
        }
        StatusCode::UNAUTHORIZED => Error::LoginRequired,
        StatusCode::FORBIDDEN => Error::Forbidden {
            msg: format!("You have no permissions to view {}", resource),
        },
        StatusCode::NOT_FOUND => not_found,
        _ => Error::Service {
            msg: "Service error. Try again later.".to_string(),
        },
    }
}

pub async fn parse_response_error(response: reqwest::Response) -> Result<String> {
    let Some(content_type) = response.headers().get("Content-Type") else {
        return Err(Error::Service {
            msg: "Unable to identify service response type".to_string(),
        });
    };

    let Ok(content_type) = content_type.to_str() else {
        return Err(Error::Service {
            msg: "Unable to identify service response type".to_string(),
        });
    };

    match content_type {
        "application/json" => {
            // Expected response when properly handled by the backend service
            let json = response
                .json::<ErrorResponse>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to parse error response.",
                })?;
            Ok(json.message)
        }
        "text/plain" | "text/plain; charset=utf-8" => {
            // Probably some default http error
            let text_res = response.text().await;
            match text_res {
                Ok(text) => Ok(text),
                Err(_) => Err(Error::Service {
                    msg: "Unable to parse text service error response".to_string(),
                }),
            }
        }
        _ => Err(Error::Service {
            msg: "Unable to parse service error response".to_string(),
        }),
    }
}
