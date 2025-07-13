use memo::client::ClientDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::run::AppState;
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

pub async fn list_clients(state: &AppState, token: &str) -> Result<Vec<ClientDto>> {
    let url = format!("{}/clients", &state.config.api_url);

    let response = state
        .client
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
    state: &AppState,
    token: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == "new_client", CsrfTokenSnafu);

    let url = format!("{}/clients", &state.config.api_url);

    let data = ClientSubmitData {
        name: form.name.clone(),
        status: match form.active {
            Some(_) => "active".to_string(),
            None => "inactive".to_string(),
        },
    };
    let response = state
        .client
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

pub async fn get_client(state: &AppState, token: &str, client_id: &str) -> Result<ClientDto> {
    let url = format!("{}/clients/{}", &state.config.api_url, client_id);
    let response = state
        .client
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
    state: &AppState,
    token: &str,
    client_id: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(&csrf_result == client_id, CsrfTokenSnafu);

    let url = format!("{}/clients/{}", &state.config.api_url, client_id);
    let data = ClientSubmitData {
        name: form.name.clone(),
        status: match form.active {
            Some(_) => "active".to_string(),
            None => "inactive".to_string(),
        },
    };
    let response = state
        .client
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
    state: &AppState,
    token: &str,
    client_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == client_id, CsrfTokenSnafu);

    let url = format!("{}/clients/{}", &state.config.api_url, client_id);
    let response = state
        .client
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
