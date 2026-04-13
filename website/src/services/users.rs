use memo::user::UserDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu, ValidationSnafu};
use crate::run::AppState;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

use super::handle_response_error;

#[derive(Clone, Deserialize, Serialize)]
pub struct NewUserFormData {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub role: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewUserData {
    pub username: String,
    pub password: String,
    pub status: String,
    pub roles: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserActiveFormData {
    pub token: String,
    pub active: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserStatusData {
    pub status: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRoleFormData {
    pub token: String,
    pub role: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRolesData {
    pub roles: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResetPasswordFormData {
    pub token: String,
    pub password: String,
    pub confirm_password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResetPasswordData {
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChangePasswordFormData {
    pub token: String,
    pub current_password: String,
    pub new_password: String,
    pub confirm_new_password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChangePasswordData {
    pub current_password: String,
    pub new_password: String,
}

pub async fn list_users(state: &AppState, token: &str, client_id: &str) -> Result<Vec<UserDto>> {
    let url = format!("{}/clients/{}/users", &state.config.api_url, client_id);

    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list users. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let users = response
        .json::<Vec<UserDto>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse users.".to_string(),
        })?;

    Ok(users)
}
