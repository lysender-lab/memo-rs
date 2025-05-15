use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDto {
    pub id: String,
    pub name: String,
    pub default_bucket_id: Option<String>,
    pub status: String,
    pub admin: bool,
    pub created_at: i64,
}
