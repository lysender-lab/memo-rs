use memo::bucket::BucketDto;
use reqwest::{Client, StatusCode};
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu, ValidationSnafu};
use crate::models::buckets::{NewBucketData, NewBucketFormData};
use crate::models::users::{
    NewUserData, NewUserFormData, ResetPasswordData, ResetPasswordFormData, UserActiveFormData,
    UserRoleFormData, UserRolesData, UserStatusData,
};
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

use super::handle_response_error;

pub async fn list_buckets(api_url: &str, token: &str, client_id: &str) -> Result<Vec<BucketDto>> {
    let url = format!("{}/clients/{}/buckets", api_url, client_id);

    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list buckets. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    let buckets = response
        .json::<Vec<BucketDto>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse buckets.".to_string(),
        })?;

    Ok(buckets)
}

pub async fn create_bucket(
    config: &Config,
    token: &str,
    client_id: &str,
    form: &NewBucketFormData,
) -> Result<BucketDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_bucket", CsrfTokenSnafu);

    let url = format!("{}/clients/{}/buckets", &config.api_url, client_id);

    let data = NewBucketData {
        name: form.name.clone(),
        images_only: match form.images_only {
            Some(_) => true,
            None => false,
        },
    };

    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create bucket. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    let bucket = response
        .json::<BucketDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse bucket information.",
        })?;

    Ok(bucket)
}

pub async fn get_user(
    api_url: &str,
    token: &str,
    client_id: &str,
    user_id: &str,
) -> Result<BucketDto> {
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
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    let user = response
        .json::<BucketDto>()
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
) -> Result<BucketDto> {
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
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    let user = response
        .json::<BucketDto>()
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
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    Ok(())
}
