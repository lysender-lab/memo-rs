use axum::body::Bytes;
use axum::http::HeaderMap;
use reqwest::{Client, StatusCode};
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, ErrorResponse, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::{
    Album, FileObject, ListAlbumsParams, ListPhotosParams, NewAlbum, NewAlbumForm, Paginated,
    Photo, UpdateAlbum, UpdateAlbumForm,
};
use crate::{Error, Result};

use super::verify_csrf_token;

pub async fn list_albums(
    api_url: &str,
    token: &str,
    bucket_id: &str,
    params: &ListAlbumsParams,
) -> Result<Paginated<Album>> {
    let url = format!("{}/v1/buckets/{}/dirs", api_url, bucket_id);
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
        return Err(handle_response_error(response).await);
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
    bucket_id: &str,
    form: NewAlbumForm,
) -> Result<Album> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_album", CsrfTokenSnafu);

    let url = format!("{}/v1/buckets/{}/dirs", &config.api_url, bucket_id);

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
        return Err(handle_response_error(response).await);
    }

    let album = response
        .json::<Album>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse album information.",
        })?;

    Ok(album)
}

pub async fn get_album(
    api_url: &str,
    token: &str,
    bucket_id: &str,
    album_id: &str,
) -> Result<Album> {
    let url = format!("{}/v1/buckets/{}/dirs/{}", api_url, bucket_id, album_id);
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get album. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let album = response
        .json::<Album>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse album.",
        })?;

    Ok(album)
}

pub async fn update_album(
    config: &Config,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    form: &UpdateAlbumForm,
) -> Result<Album> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == album_id, CsrfTokenSnafu);

    let url = format!(
        "{}/v1/buckets/{}/dirs/{}",
        &config.api_url, bucket_id, album_id
    );
    let data = UpdateAlbum {
        label: form.label.clone(),
    };
    let response = Client::new()
        .patch(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update album. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let album = response
        .json::<Album>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse album information.",
        })?;

    Ok(album)
}

pub async fn delete_album(
    config: &Config,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == album_id, CsrfTokenSnafu);
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}",
        &config.api_url, bucket_id, album_id
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

pub async fn list_photos(
    api_url: &str,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    params: &ListPhotosParams,
) -> Result<Paginated<Photo>> {
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}/files",
        api_url, bucket_id, album_id
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
            msg: "Unable to list photos. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let listing =
        response
            .json::<Paginated<FileObject>>()
            .await
            .context(HttpResponseParseSnafu {
                msg: "Unable to parse photos.".to_string(),
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

pub async fn upload_photo(
    config: &Config,
    token: &str,
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
        "{}/v1/buckets/{}/dirs/{}/files",
        &config.api_url, bucket_id, album_id
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
        return Err(handle_response_error(response).await);
    }

    let file = response
        .json::<FileObject>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse photo information.".to_string(),
        })?;

    Ok(Photo::try_from(file)?)
}

pub async fn get_photo(
    api_url: &str,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
) -> Result<Photo> {
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}/files/{}",
        api_url, bucket_id, album_id, photo_id
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
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    ensure!(csrf_result == photo_id, CsrfTokenSnafu);
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}/files/{}",
        &config.api_url, bucket_id, album_id, photo_id
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

pub async fn parse_response_error(response: reqwest::Response) -> Result<String> {
    let Some(content_type) = response.headers().get("Content-Type") else {
        return Err(Error::Service {
            msg: "Unable to identify service response type".to_string(),
        });
    };

    let Ok(content_type) = content_type.to_str() else {
        return Err(Error::Service {
            msg: "Unable to identify service response type".to_string(),
        });
    };

    match content_type {
        "application/json" => {
            // Expected response when properly handled by the backend service
            let json = response
                .json::<ErrorResponse>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to parse error response.",
                })?;
            Ok(json.message)
        }
        "text/plain" | "text/plain; charset=utf-8" => {
            // Probably some default http error
            let text_res = response.text().await;
            match text_res {
                Ok(text) => Ok(text),
                Err(_) => Err(Error::Service {
                    msg: "Unable to parse text service error response".to_string(),
                }),
            }
        }
        _ => Err(Error::Service {
            msg: "Unable to parse service error response".to_string(),
        }),
    }
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
            msg: "You have no permissions to view albums".to_string(),
        },
        StatusCode::NOT_FOUND => Error::AlbumNotFound,
        _ => Error::Service {
            msg: "Service error. Try again later.".to_string(),
        },
    }
}
