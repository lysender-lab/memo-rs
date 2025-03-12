use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Params {
    pub bucket_id: String,
    pub dir_id: Option<String>,
    pub file_id: Option<String>,
}
