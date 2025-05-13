use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Params {
    pub bucket_id: String,
    pub dir_id: Option<String>,
    pub file_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientParams {
    pub client_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BucketParams {
    pub client_id: String,
    pub bucket_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DirParams {
    pub client_id: String,
    pub bucket_id: String,
    pub dir_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FileParams {
    pub client_id: String,
    pub bucket_id: String,
    pub dir_id: String,
    pub file_id: String,
}
