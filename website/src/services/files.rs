use axum::body::Bytes;
use axum::http::HeaderMap;
use memo::file::{FileDto, ImgDimension, ImgVersion};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};

use crate::error::{CsrfTokenSnafu, HttpClientSnafu, HttpResponseParseSnafu};
use crate::models::ListFilesParams;
use crate::run::AppState;
use crate::services::handle_response_error;
use crate::services::token::verify_csrf_token;
use crate::{Error, Result};
use memo::pagination::Paginated;

#[derive(Clone, Deserialize, Serialize)]
pub struct Photo {
    pub id: String,
    pub dir_id: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub orig: PhotoVersionDto,
    pub preview: PhotoVersionDto,
    pub thumb: PhotoVersionDto,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PhotoVersionDto {
    pub version: ImgVersion,
    pub dimension: ImgDimension,
    pub url: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct UploadResult {
    pub error_message: Option<String>,
    pub photo: Option<Photo>,
    pub next_token: String,
}

impl TryFrom<FileDto> for Photo {
    type Error = String;

    fn try_from(file: FileDto) -> core::result::Result<Self, Self::Error> {
        if !file.is_image {
            return Err("File is not an image".into());
        }

        let Some(versions) = file.img_versions else {
            return Err("Missing image versions".into());
        };

        let versions: Vec<PhotoVersionDto> = versions
            .into_iter()
            .filter_map(|v| match v.url {
                None => None,
                Some(url) => Some(PhotoVersionDto {
                    version: v
                        .version
                        .to_string()
                        .as_str()
                        .try_into()
                        .expect("Photo version must be valid"),
                    dimension: v.dimension,
                    url,
                }),
            })
            .collect();

        let orig = versions.iter().find(|v| v.version == ImgVersion::Original);
        let mut preview = versions.iter().find(|v| v.version == ImgVersion::Preview);
        let thumb = versions.iter().find(|v| v.version == ImgVersion::Thumbnail);

        if preview.is_none() && orig.is_some() {
            preview = orig.clone();
        }

        if orig.is_none() || preview.is_none() || thumb.is_none() {
            return Err("Missing image versions".into());
        }

        Ok(Photo {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            orig: orig.expect("orig version must be present").clone(),
            preview: preview.expect("preview version must be present").clone(),
            thumb: thumb.expect("thumb version must be present").clone(),
            created_at: file.created_at,
            updated_at: file.updated_at,
        })
    }
}

pub async fn list_files(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    dir_id: &str,
    params: &ListFilesParams,
) -> Result<Paginated<Photo>> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files",
        &state.config.api_url, client_id, bucket_id, dir_id
    );
    let mut page = "1".to_string();
    let per_page = "50".to_string();

    if let Some(p) = params.page {
        page = p.to_string();
    }
    let query: Vec<(&str, &str)> = vec![("page", &page), ("per_page", &per_page)];
    let response = state
        .client
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

    let listing = response
        .json::<Paginated<FileDto>>()
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
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
) -> Result<Photo> {
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files/{}",
        &state.config.api_url, client_id, bucket_id, album_id, photo_id
    );
    let response = state
        .client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to get photo. Try again later.".to_string(),
        })?;

    let file = response
        .json::<FileDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse photo.".to_string(),
        })?;

    Ok(Photo::try_from(file)?)
}

pub async fn upload_photo(
    state: &AppState,
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
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == album_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files",
        &state.config.api_url, client_id, bucket_id, album_id
    );

    let response = state
        .client
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
        .json::<FileDto>()
        .await
        .context(HttpResponseParseSnafu {
            msg: "Unable to parse photo information.".to_string(),
        })?;

    Ok(Photo::try_from(file)?)
}

pub async fn delete_photo(
    state: &AppState,
    token: &str,
    client_id: &str,
    bucket_id: &str,
    album_id: &str,
    photo_id: &str,
    csrf_token: &str,
) -> Result<()> {
    let csrf_result = verify_csrf_token(&csrf_token, &state.config.jwt_secret)?;
    ensure!(csrf_result == photo_id, CsrfTokenSnafu);
    let url = format!(
        "{}/clients/{}/buckets/{}/dirs/{}/files/{}",
        &state.config.api_url, client_id, bucket_id, album_id, photo_id
    );
    let _ = state
        .client
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to delete photo. Try again later.".to_string(),
        })?;

    Ok(())
}
