use memo::dir::DirDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::run::AppState;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};
use memo::pagination::Paginated;

use super::handle_response_error;

#[derive(Deserialize)]
pub struct SearchDirsParams {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NewDirFormData {
    pub name: String,
    pub label: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewDirData {
    pub name: String,
    pub label: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct UpdateDirFormData {
    pub label: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct UpdateDirData {
    pub label: String,
}

pub async fn list_dirs(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    params: &SearchDirsParams,
) -> Result<Paginated<DirDto>> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs",
        &state.config.api_url, client_id, bucket_id
    );
    let mut page = "1".to_string();
    let mut per_page = "10".to_string();

    if let Some(p) = params.page {
        page = p.to_string();
    }
    if let Some(pp) = params.per_page {
        per_page = pp.to_string();
    }
    let mut query: Vec<(&str, &str)> = vec![("page", &page), ("per_page", &per_page)];
    if let Some(keyword) = &params.keyword {
        query.push(("keyword", keyword));
    }
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list dirs. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "dirs", Error::AlbumNotFound).await);
    }

    let dirs = response
        .json::<Paginated<DirDto>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse dirs.".to_string(),
        })?;

    Ok(dirs)
}

pub async fn create_dir(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    form: NewDirFormData,
) -> Result<DirDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == "new_dir", CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/buckets/{}/dirs",
        &state.config.api_url, client_id, bucket_id
    );

    let data = NewDirData {
        name: form.name,
        label: form.label,
    };
    let response = state
        .client
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create dir. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "dirs", Error::BucketNotFound).await);
    }

    let dir = response
        .json::<DirDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse dir information.",
        })?;

    Ok(dir)
}

pub async fn get_dir(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    dir_id: &str,
) -> Result<DirDto> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}",
        &state.config.api_url, client_id, bucket_id, dir_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get dir. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "dirs", Error::AlbumNotFound).await);
    }

    let dir = response
        .json::<DirDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse dir.",
        })?;

    Ok(dir)
}

pub async fn update_dir(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    dir_id: &str,
    form: &UpdateDirFormData,
) -> Result<DirDto> {
    let csrf_result = verify_csrf_token(&form.token, &state.config.jwt_secret)?;
    ensure!(csrf_result == dir_id, CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}",
        &state.config.api_url, client_id, bucket_id, dir_id
    );
    let data = UpdateDirData {
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
            msg: "Unable to update dir. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "dirs", Error::AlbumNotFound).await);
    }

    let dir = response
        .json::<DirDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse dir information.",
        })?;

    Ok(dir)
}

pub async fn delete_dir(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    dir_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == dir_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}",
        &state.config.api_url, client_id, bucket_id, dir_id
    );
    let response = state
        .client
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete dir. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "dirs", Error::AlbumNotFound).await);
    }

    Ok(())
}
