use memo::user::UserDto;
use reqwest::{Client, StatusCode};
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu, ValidationSnafu};
use crate::models::users::{
    NewUserData, NewUserFormData, ResetPasswordData, ResetPasswordFormData, UserActiveFormData,
    UserRoleFormData, UserRolesData, UserStatusData,
};
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

use super::clients::parse_response_error;

pub async fn list_users(api_url: &str, token: &str, client_id: &str) -> Result<Vec<UserDto>> {
    let url = format!("{}/clients/{}/users", api_url, client_id);

    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list users. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
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
    config: &Config,
    token: &str,
    client_id: &str,
    form: &NewUserFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_user", CsrfTokenSnafu);

    ensure!(
        form.password.as_str() == form.confirm_password.as_str(),
        ValidationSnafu {
            msg: "Passwords must match".to_string()
        }
    );

    let url = format!("{}/clients/{}/users", &config.api_url, client_id);

    let data = NewUserData {
        username: form.username.clone(),
        password: form.password.clone(),
        status: "active".to_string(),
        roles: form.role.clone(),
    };

    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create user. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
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
    api_url: &str,
    token: &str,
    client_id: &str,
    user_id: &str,
) -> Result<UserDto> {
    let url = format!("{}/clients/{}/users/{}", api_url, client_id, user_id);
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
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
    config: &Config,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &UserActiveFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/users/{}/update_status",
        &config.api_url, client_id, user_id
    );
    let data = UserStatusData {
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
            msg: "Unable to update user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
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
    config: &Config,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &UserRoleFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/users/{}/update_roles",
        &config.api_url, client_id, user_id
    );
    let data = UserRolesData {
        roles: form.role.clone(),
    };

    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
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
    config: &Config,
    token: &str,
    client_id: &str,
    user_id: &str,
    form: &ResetPasswordFormData,
) -> Result<UserDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(&csrf_result == user_id, CsrfTokenSnafu);

    ensure!(
        &form.password == &form.confirm_password,
        ValidationSnafu {
            msg: "Passwords must match."
        }
    );

    let url = format!(
        "{}/clients/{}/users/{}/reset_password",
        &config.api_url, client_id, user_id
    );

    let data = ResetPasswordData {
        password: form.password.clone(),
    };

    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update user. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let user = response
        .json::<UserDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse user information.",
        })?;

    Ok(user)
}

pub async fn delete_album(
    config: &Config,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == album_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}",
        &config.api_url, client_id, bucket_id, album_id
    );
    let response = Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete album. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    Ok(())
}

async fn handle_response_error(response: reqwest::Response) -> Error {
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
            msg: "You have no permissions to view users".to_string(),
        },
        StatusCode::NOT_FOUND => Error::UserNotFound,
        _ => Error::Service {
            msg: "Service error. Try again later.".to_string(),
        },
    }
}
