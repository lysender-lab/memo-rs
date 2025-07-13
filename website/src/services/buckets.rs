use memo::bucket::BucketDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::run::AppState;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

use super::handle_response_error;

#[derive(Clone, Deserialize, Serialize)]
pub struct NewBucketFormData {
    pub name: String,
    pub label: String,
    pub images_only: Option<String>,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewBucketData {
    pub name: String,
    pub label: String,
    pub images_only: bool,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct UpdateBucketFormData {
    pub label: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct UpdateBucketData {
    pub label: String,
}

pub async fn list_buckets(
    state: &AppState,
    token: &str,
    client_id: &str,
) -> Result<Vec<BucketDto>> {
    let url = format!("{}/clients/{}/buckets", &state.config.api_url, client_id);

    let response = state
        .client
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
    state: &AppState,
    token: &str,
    client_id: &str,
    form: &NewBucketFormData,
) -> Result<BucketDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == "new_bucket", CsrfTokenSnafu);

    let url = format!("{}/clients/{}/buckets", &state.config.api_url, client_id);

    let data = NewBucketData {
        name: form.name.clone(),
        label: form.label.clone(),
        images_only: match form.images_only {
            Some(_) => true,
            None => false,
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

pub async fn get_bucket(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
) -> Result<BucketDto> {
    let url = format!(
        "{}/clients/{}/buckets/{}",
        &state.config.api_url, client_id, bucket_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get bucket. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    let user = response
        .json::<BucketDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse bucket.",
        })?;

    Ok(user)
}

pub async fn update_bucket(
    state: &AppState,
    token: &str,
    client_id: &str,
    id: &str,
    form: &UpdateBucketFormData,
) -> Result<BucketDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/buckets/{}",
        &state.config.api_url, client_id, id
    );

    let data = UpdateBucketData {
        label: form.label.clone(),
    };

    let response = state
        .client
        .patch(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update bucket. Try again later.".to_string(),
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

pub async fn delete_bucket(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == bucket_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}",
        &state.config.api_url, client_id, bucket_id
    );
    let response = state
        .client
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete bucket. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "buckets", Error::BucketNotFound).await);
    }

    Ok(())
}
