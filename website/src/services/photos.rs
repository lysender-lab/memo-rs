use axum::body::Bytes;
use axum::http::HeaderMap;
use reqwest::Client;
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::{
    Album, FileObject, ListAlbumsParams, ListPhotosParams, NewAlbum, NewAlbumForm, Photo,
    UpdateAlbum, UpdateAlbumForm,
};
use crate::services::handle_response_error;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};
use memo::pagination::Paginated;

pub async fn list_albums(
    api_url: &str,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    params: &ListAlbumsParams,
) -> Result<Paginated<Album>> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs",
        api_url, client_id, bucket_id
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
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to list albums. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "albums", Error::FileNotFound).await);
    }

    let albums = response
        .json::<Paginated<Album>>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse albums.".to_string(),
        })?;

    Ok(albums)
}

pub async fn create_album(
    config: &Config,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    form: NewAlbumForm,
) -> Result<Album> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_album", CsrfTokenSnafu);

    let url = format!(
        "{}/clients/{}/buckets/{}/dirs",
        &config.api_url, client_id, bucket_id
    );

    let data = NewAlbum {
        name: form.name,
        label: form.label,
    };
    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create album. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response, "albums", Error::FileNotFound).await);
    }

    let album = response
        .json::<Album>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse album information.",
        })?;

    Ok(album)
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
