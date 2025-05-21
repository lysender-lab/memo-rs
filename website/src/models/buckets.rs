use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct NewBucketFormData {
    pub name: String,
    pub images_only: Option<String>,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewBucketData {
    pub name: String,
    pub images_only: bool,
}
