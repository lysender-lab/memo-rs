use memo::client::ClientDto;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

use super::handle_response_error;

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientFormSubmitData {
    pub name: String,
    pub active: Option<String>,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct ClientSubmitData {
    pub name: String,
    pub status: String,
}

pub async fn list_clients(api_url: &str, token: &str) -> Result<Vec<ClientDto>> {
    let url = format!("{}/clients", api_url);

    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list clients. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "clients", Error::ClientNotFound).await);
    }

    let clients = response
        .json::<Vec<ClientDto>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse clients.".to_string(),
        })?;

    Ok(clients)
}

pub async fn create_client(
    config: &Config,
    token: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_client", CsrfTokenSnafu);

    let url = format!("{}/clients", &config.api_url);

    let data = ClientSubmitData {
        name: form.name.clone(),
        status: match form.active {
            Some(_) => "active".to_string(),
            None => "inactive".to_string(),
        },
    };
    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create client. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "clients", Error::ClientNotFound).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client information.",
        })?;

    Ok(client)
}

pub async fn get_client(api_url: &str, token: &str, client_id: &str) -> Result<ClientDto> {
    let url = format!("{}/clients/{}", api_url, client_id);
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get client. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "clients", Error::ClientNotFound).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client.",
        })?;

    Ok(client)
}

pub async fn update_client(
    config: &Config,
    token: &str,
    client_id: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(&csrf_result == client_id, CsrfTokenSnafu);

    let url = format!("{}/clients/{}", &config.api_url, client_id);
    let data = ClientSubmitData {
        name: form.name.clone(),
        status: match form.active {
            Some(_) => "active".to_string(),
            None => "inactive".to_string(),
        },
    };
    let response = Client::new()
        .patch(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update client. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "clients", Error::ClientNotFound).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client information.",
        })?;

    Ok(client)
}

pub async fn delete_client(
    config: &Config,
    token: &str,
    client_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == client_id, CsrfTokenSnafu);

    let url = format!("{}/clients/{}", &config.api_url, client_id);
    let response = Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete client. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "clients", Error::ClientNotFound).await);
    }

    Ok(())
}
