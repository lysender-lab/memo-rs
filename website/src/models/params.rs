use core::fmt;
use urlencoding::encode;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct AlbumParams {
    pub album_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ListAlbumsParams {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Deserialize)]
pub struct UploadParams {
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct ListPhotosParams {
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct PhotoParams {
    pub album_id: Option<String>,
    pub photo_id: Option<String>,
}

impl Default for ListAlbumsParams {
    fn default() -> Self {
        Self {
            keyword: None,
            page: Some(1),
            per_page: Some(10),
        }
    }
}

impl fmt::Display for ListAlbumsParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Ideally, we want an empty string if all fields are None
        if self.keyword.is_none() && self.page.is_none() && self.per_page.is_none() {
            return write!(f, "");
        }

        let keyword = self.keyword.as_deref().unwrap_or("");
        let page = self.page.unwrap_or(1);
        let per_page = self.per_page.unwrap_or(10);

        write!(
            f,
            "page={}&per_page={}&keyword={}",
            page,
            per_page,
            encode(keyword)
        )
    }
}
