use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirDto {
    pub id: String,
    pub bucket_id: String,
    pub name: String,
    pub label: String,
    pub file_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
