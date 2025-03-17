use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ClientDto {
    pub id: String,
    pub name: String,
    pub default_bucket_id: Option<String>,
    pub status: String,
    pub admin: bool,
    pub created_at: i64,
}
