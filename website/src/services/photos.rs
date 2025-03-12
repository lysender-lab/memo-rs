use axum::body::Bytes;
use axum::http::HeaderMap;
use reqwest::{Client, StatusCode};
use tracing::error;

use crate::config::Config;
use crate::error::ErrorResponse;
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
    let result = Client::new()
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to list albums. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => {
            let json_res = response.json::<Paginated<Album>>().await;
            match json_res {
                Ok(albums) => return Ok(albums),
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::JsonParseError("Unable to parse albums.".to_string()));
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permissions to view albums".to_string(),
        )),
        StatusCode::NOT_FOUND => Err(Error::AlbumNotFound),
        _ => Err(Error::ServiceError(
            "Unable to list albums. Try again later.".to_string(),
        )),
    }
}

pub async fn create_album(
    config: &Config,
    token: &str,
    bucket_id: &str,
    form: NewAlbumForm,
) -> Result<Album> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    if csrf_result != "new_album" {
        return Err(Error::InvalidCsrfToken);
    }
    let url = format!("{}/v1/buckets/{}/dirs", &config.api_url, bucket_id);

    let data = NewAlbum {
        name: form.name,
        label: form.label,
    };
    let result = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to create album. Try again later.".into());
    };

    match response.status() {
        StatusCode::CREATED => {
            let json_res = response.json::<Album>().await;
            match json_res {
                Ok(album) => return Ok(album),
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::JsonParseError(
                        "Unable to parse album information.".to_string(),
                    ));
                }
            }
        }
        StatusCode::BAD_REQUEST => {
            let json_res = response.json::<ErrorResponse>().await;
            match json_res {
                Ok(json) => {
                    return Err(Error::ValidationError(json.message));
                }
                Err(e) => {
                    // Most likely a bad request not handled by the API
                    error!("Error: {}", e);
                    return Err(Error::BadRequest("Bad Request.".to_string()));
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to create new albums.".to_string(),
        )),
        _ => Err(Error::ServiceError(
            "Unable to create album. Try again later.".to_string(),
        )),
    }
}

pub async fn get_album(
    api_url: &str,
    token: &str,
    bucket_id: &str,
    album_id: &str,
) -> Result<Album> {
    let url = format!("{}/v1/buckets/{}/dirs/{}", api_url, bucket_id, album_id);
    let result = Client::new().get(url).bearer_auth(token).send().await;

    let Ok(response) = result else {
        return Err("Unable to get album. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => {
            let json_res = response.json::<Album>().await;
            match json_res {
                Ok(album) => return Ok(album),
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::JsonParseError("Unable to parse album.".to_string()));
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to read this album.".to_string(),
        )),
        StatusCode::NOT_FOUND => Err(Error::AlbumNotFound),
        _ => Err(Error::ServiceError(
            "Unable to get album. Try again later.".to_string(),
        )),
    }
}

pub async fn update_album(
    config: &Config,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    form: &UpdateAlbumForm,
) -> Result<Album> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    if csrf_result != album_id {
        return Err(Error::InvalidCsrfToken);
    }
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}",
        &config.api_url, bucket_id, album_id
    );
    let data = UpdateAlbum {
        label: form.label.clone(),
    };
    let result = Client::new()
        .patch(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to update album. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => {
            let json_res = response.json::<Album>().await;
            match json_res {
                Ok(album) => return Ok(album),
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::JsonParseError(
                        "Unable to parse album information.".to_string(),
                    ));
                }
            }
        }
        StatusCode::BAD_REQUEST => {
            let json_res = response.json::<ErrorResponse>().await;
            match json_res {
                Ok(json) => {
                    return Err(Error::ValidationError(json.message));
                }
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::BadRequest("Bad Request.".to_string()));
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::ServiceError(
            "You have no permission to update this album.".to_string(),
        )),
        _ => Err(Error::ServiceError(
            "Unable to update album. Try again later.".to_string(),
        )),
    }
}

pub async fn delete_album(
    config: &Config,
    token: &str,
    bucket_id: &str,
    album_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &config.jwt_secret)?;
    if csrf_result != album_id {
        return Err(Error::InvalidCsrfToken);
    }
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}",
        &config.api_url, bucket_id, album_id
    );
    let result = Client::new().delete(url).bearer_auth(token).send().await;

    let Ok(response) = result else {
        return Err("Unable to delete album. Try again later.".into());
    };

    match response.status() {
        StatusCode::NO_CONTENT => Ok(()),
        StatusCode::BAD_REQUEST => {
            let json_res = response.json::<ErrorResponse>().await;
            match json_res {
                Ok(json) => {
                    return Err(Error::ValidationError(json.message));
                }
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::BadRequest("Bad Request.".to_string()));
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to delete this album.".to_string(),
        )),
        _ => Err(Error::ServiceError(
            "Unable to delete album. Try again later.".to_string(),
        )),
    }
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
    let result = Client::new()
        .get(url)
        .bearer_auth(token)
        .query(&query)
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to list photos. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => {
            let json_res = response.json::<Paginated<FileObject>>().await;
            match json_res {
                Ok(listing) => {
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
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::JsonParseError("Unable to parse photos.".to_string()))
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permissions to view photos".to_string(),
        )),
        StatusCode::NOT_FOUND => Err(Error::AlbumNotFound),
        _ => Err(Error::ServiceError(
            "Unable to list photos. Try again later.".to_string(),
        )),
    }
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
    if csrf_result != album_id {
        return Err(Error::InvalidCsrfToken);
    }
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}/files",
        &config.api_url, bucket_id, album_id
    );

    let result = Client::new()
        .post(url)
        .header("Content-Type", content_type)
        .header("Content-Length", body.len().to_string())
        .bearer_auth(token)
        .body(body)
        .send()
        .await;

    let Ok(response) = result else {
        return Err("Unable to upload photo. Try again later.".into());
    };

    match response.status() {
        StatusCode::CREATED => {
            let json_res = response.json::<FileObject>().await;
            match json_res {
                Ok(file) => Ok(Photo::try_from(file)?),
                Err(e) => {
                    error!("Error: {}", e);
                    return Err(Error::JsonParseError(
                        "Unable to parse photo information.".to_string(),
                    ));
                }
            }
        }
        StatusCode::BAD_REQUEST => {
            let message_res = parse_response_error(response).await;
            match message_res {
                Ok(msg) => Err(Error::ValidationError(msg)),
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::BadRequest("Bad Request.".to_string()))
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to upload photos.".to_string(),
        )),
        _ => Err(Error::ServiceError(
            "Unable to upload photo. Try again later.".to_string(),
        )),
    }
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
    let result = Client::new().get(url).bearer_auth(token).send().await;

    let Ok(response) = result else {
        return Err("Unable to get photo. Try again later.".into());
    };

    match response.status() {
        StatusCode::OK => {
            let json_res = response.json::<FileObject>().await;
            match json_res {
                Ok(file) => Ok(Photo::try_from(file)?),
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::JsonParseError("Unable to parse photo.".to_string()))
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to read this photo.".to_string(),
        )),
        StatusCode::NOT_FOUND => Err(Error::PhotoNotFound),
        _ => Err(Error::ServiceError(
            "Unable to get photo. Try again later.".to_string(),
        )),
    }
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
    if csrf_result != photo_id {
        return Err(Error::InvalidCsrfToken);
    }
    let url = format!(
        "{}/v1/buckets/{}/dirs/{}/files/{}",
        &config.api_url, bucket_id, album_id, photo_id
    );
    let result = Client::new().delete(url).bearer_auth(token).send().await;

    let Ok(response) = result else {
        return Err("Unable to delete photo. Try again later.".into());
    };

    match response.status() {
        StatusCode::NO_CONTENT => Ok(()),
        StatusCode::BAD_REQUEST => {
            let message_res = parse_response_error(response).await;
            match message_res {
                Ok(msg) => Err(Error::ValidationError(msg)),
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::BadRequest("Bad Request.".to_string()))
                }
            }
        }
        StatusCode::UNAUTHORIZED => Err(Error::LoginRequired("Login first".to_string())),
        StatusCode::FORBIDDEN => Err(Error::Forbidden(
            "You have no permission to delete this photo.".to_string(),
        )),
        _ => Err(Error::ServiceError(
            "Unable to delete photo. Try again later.".to_string(),
        )),
    }
}

async fn parse_response_error(response: reqwest::Response) -> Result<String> {
    let Some(content_type) = response.headers().get("Content-Type") else {
        return Err(Error::ServiceError(
            "Unable to identify service response type".to_string(),
        ));
    };

    let Ok(content_type) = content_type.to_str() else {
        return Err(Error::ServiceError(
            "Unable to identify service response type".to_string(),
        ));
    };

    match content_type {
        "application/json" => {
            // Expected response when properly handled by the backend service
            let json_res = response.json::<ErrorResponse>().await;
            match json_res {
                Ok(json) => Ok(json.message),
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::ServiceError(
                        "Unable to parse JSON service error response".to_string(),
                    ))
                }
            }
        }
        "text/plain" | "text/plain; charset=utf-8" => {
            // Probably some default http error
            let text_res = response.text().await;
            match text_res {
                Ok(text) => Ok(text),
                Err(e) => {
                    error!("Error: {}", e);
                    Err(Error::ServiceError(
                        "Unable to parse text service error response".to_string(),
                    ))
                }
            }
        }
        _ => Err(Error::ServiceError(
            "Unable to parse service error response".to_string(),
        )),
    }
}
