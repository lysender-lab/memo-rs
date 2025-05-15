use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketDto {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub images_only: bool,
    pub created_at: i64,
}
