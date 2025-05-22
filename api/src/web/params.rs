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
pub struct UserParams {
    #[allow(dead_code)]
    pub client_id: String,

    pub user_id: String,
}
