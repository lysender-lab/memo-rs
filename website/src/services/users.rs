use axum::body::Bytes;
use axum::http::HeaderMap;
use memo::client::ClientDto;
use memo::user::UserDto;
use reqwest::{Client, StatusCode};
use snafu::{ResultExt, ensure};

use crate::config::Config;
use crate::error::{CsrfTokenSnafu, ErrorResponse, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::clients::{ClientFormSubmitData, ClientSubmitData};
use crate::models::{
    Album, FileObject, ListAlbumsParams, ListPhotosParams, NewAlbum, NewAlbumForm, Paginated,
    Photo, UpdateAlbum, UpdateAlbumForm,
};
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};

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

pub async fn create_client(
    config: &Config,
    token: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(csrf_result == "new_client", CsrfTokenSnafu);

    let url = format!("{}/clients", &config.api_url);

    let data = ClientSubmitData {
        name: form.name.clone(),
        status: form.status.clone(),
    };
    let response = Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to create client. Try again later.".to_string(),
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client information.",
        })?;

    Ok(client)
}

pub async fn get_client(api_url: &str, token: &str, client_id: &str) -> Result<ClientDto> {
    let url = format!("{}/clients/{}", api_url, client_id);
    let response = Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get client. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client.",
        })?;

    Ok(client)
}

pub async fn update_client(
    config: &Config,
    token: &str,
    client_id: &str,
    form: &ClientFormSubmitData,
) -> Result<ClientDto> {
    let csrf_result = verify_csrf_token(&form.token, &config.jwt_secret)?;
    ensure!(&csrf_result == client_id, CsrfTokenSnafu);

    let url = format!("{}/clients/{}", &config.api_url, client_id);
    let data = ClientSubmitData {
        name: form.name.clone(),
        status: form.status.clone(),
    };
    let response = Client::new()
        .patch(url)
        .bearer_auth(token)
        .json(&data)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to update client. Try again later.",
        })?;

    if !response.status().is_success() {
        return Err(handle_response_error(response).await);
    }

    let client = response
        .json::<ClientDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse client information.",
        })?;

    Ok(client)
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

pub async fn list_photos(
    api_url: &str,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    params: &ListPhotosParams,
) -> Result<Paginated<Photo>> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files",
        api_url, client_id, bucket_id, album_id
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
