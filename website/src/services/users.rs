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

pub async fn create_user(
    state: &AppState,
    token: &str,
    client_id: &str,
    form: &NewUserFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == "new_user", CsrfTokenSnafu);

    ensure!(
        form.password.as_str() == form.confirm_password.as_str(),
        ValidationSnafu {
            msg: "Passwords must match".to_string()
        }
    );

    let url = format!("{}/clients/{}/users", &state.config.api_url, client_id);

    let data = NewUserData {
        username: form.username.clone(),
        password: form.password.clone(),
        status: "active".to_string(),
        roles: form.role.clone(),
    };

    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create user. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.",
        })?;

    Ok(user)
}

pub async fn get_user(
    state: &AppState,
    token: &str,
    client_id: &str,
    user_id: &str,
) -> Result<UserDto> {
    let url = format!(
        "{}/clients/{}/users/{}",
        &state.config.api_url, client_id, user_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user.",
        })?;

    Ok(user)
}

pub async fn update_user_status(
    state: &AppState,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &UserActiveFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/users/{}/update_status",
        &state.config.api_url, client_id, user_id
    );
    let data = UserStatusData {
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
            msg: "Unable to update user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.",
        })?;

    Ok(user)
}

pub async fn update_user_roles(
    state: &AppState,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &UserRoleFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/users/{}/update_roles",
        &state.config.api_url, client_id, user_id
    );
    let data = UserRolesData {
        roles: form.role.clone(),
    };

    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.",
        })?;

    Ok(user)
}

pub async fn reset_user_password(
    state: &AppState,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &ResetPasswordFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    ensure!(
        &form.password == &form.confirm_password,
        ValidationSnafu {
            msg: "Passwords must match."
        }
    );

    let url = format!(
        "{}/clients/{}/users/{}/reset_password",
        &state.config.api_url, client_id, user_id
    );

    let data = ResetPasswordData {
        password: form.password.clone(),
    };

    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update user password. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.",
        })?;

    Ok(user)
}

pub async fn change_user_password(
    state: &AppState,
    token: &str,
    user_id: &str,
    form: &ChangePasswordFormData,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    ensure!(
        &form.new_password == &form.confirm_new_password,
        ValidationSnafu {
            msg: "Passwords must match."
        }
    );

    let url = format!("{}/user/change_password", &state.config.api_url);

    let data = ChangePasswordData {
        current_password: form.current_password.clone(),
        new_password: form.new_password.clone(),
    };

    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update user password. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    Ok(())
}

pub async fn delete_user(
    state: &AppState,
    token: &str,
    client_id: &str,
    user_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == user_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/users/{}",
        &state.config.api_url, client_id, user_id,
    );
    let response = state
        .client
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete user. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "users", Error::UserNotFound).await);
    }

    Ok(())
}
