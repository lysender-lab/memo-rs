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
