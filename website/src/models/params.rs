use core::fmt;
use urlencoding::encode;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClientParams {
    pub client_id: String,
}

#[derive(Deserialize)]
pub struct UserParams {
    pub client_id: String,
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct BucketParams {
    pub client_id: String,
    pub bucket_id: String,
}

#[derive(Deserialize)]
pub struct MyBucketParams {
    pub bucket_id: String,
}

#[derive(Deserialize)]
pub struct MyDirParams {
    #[allow(dead_code)]
    pub bucket_id: String,

    pub dir_id: String,
}

#[derive(Deserialize)]
pub struct MyFileParams {
    #[allow(dead_code)]
    pub bucket_id: String,

    #[allow(dead_code)]
    pub dir_id: String,

    pub file_id: String,
}

#[derive(Deserialize)]
pub struct ListDirsParams {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Deserialize)]
pub struct UploadParams {
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct ListFilesParams {
    pub page: Option<u32>,
}

impl Default for ListDirsParams {
    fn default() -> Self {
        Self {
            keyword: None,
            page: Some(1),
            per_page: Some(10),
        }
    }
}

impl fmt::Display for ListDirsParams {
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
