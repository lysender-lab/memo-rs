use axum::body::Bytes;
use axum::http::HeaderMap;
use memo::file::FileDto;
use reqwest::Client;
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::{
    Album, FileObject, ListAlbumsParams, ListFilesParams, ListPhotosParams, NewAlbum, NewAlbumForm,
    Photo, UpdateAlbum, UpdateAlbumForm,
};
use crate::services::handle_response_error;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};
use memo::pagination::Paginated;

pub async fn list_files(
    api_url: &str,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    dir_id: &str,
    params: &ListFilesParams,
) -> Result<Paginated<Photo>> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files",
        api_url, client_id, bucket_id, dir_id
    );
    let mut page = "1".to_string();
    let per_page = "50".to_string();

    if let Some(p) = params.page {
        page = p.to_string();
    }
    let query: Vec<(&str, &str)> = vec![("page", &page), ("per_page", &per_page)];
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list files. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "files", Error::AlbumNotFound).await);
    }

    let listing =
        response
            .json::<Paginated<FileObject>>()
            .await
            .context(HttpResponseParseSnafu {
                msg: "Unable to parse files.".to_string(),
            })?;

    let items: Vec<Photo> = listing
        .data
        .into_iter()
        .filter_map(|file| file.try_into().ok())
        .collect();

    Ok(Paginated {
        meta: listing.meta,
        data: items,
    })
}

pub async fn get_photo(
    api_url: &str,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
) -> Result<Photo> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files/{}",
        api_url, client_id, bucket_id, album_id, photo_id
    );
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get photo. Try again later.".to_string(),
        })?;

    let file = response
        .json::<FileObject>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse photo.".to_string(),
        })?;

    Ok(Photo::try_from(file)?)
}

pub async fn upload_photo(
    config: &Config,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    headers: &HeaderMap,
    csrf_token: Option<String>,
    body: Bytes,
) -> Result<Photo> {
    // We need the content type header
    let Some(content_type) = headers.get("Content-Type") else {
        return Err("Content-Type header is required.".into());
    };
    let Ok(content_type) = content_type.to_str() else {
        return Err("Invalid Content-Type header.".into());
    };
    let csrf_token = csrf_token.unwrap_or("".to_string());
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == album_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files",
        &config.api_url, client_id, bucket_id, album_id
    );

    let response = Client::new()
        .post(url)
        .header("Content-Type", content_type)
        .header("Content-Length", body.len().to_string())
        .bearer_auth(token)
        .body(body)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to upload photo. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "photos", Error::FileNotFound).await);
    }

    let file = response
        .json::<FileObject>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse photo information.".to_string(),
        })?;

    Ok(Photo::try_from(file)?)
}

pub async fn delete_photo(
    config: &Config,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == photo_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files/{}",
        &config.api_url, client_id, bucket_id, album_id, photo_id
    );
    let _ = Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete photo. Try again later.".to_string(),
        })?;

    Ok(())
}
