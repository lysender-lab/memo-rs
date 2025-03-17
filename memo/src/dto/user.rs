use crate::role::Role;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    pub id: String,
    pub client_id: String,
    pub username: String,
    pub status: String,
    pub roles: Vec<Role>,
    pub created_at: i64,
    pub updated_at: i64,
}
