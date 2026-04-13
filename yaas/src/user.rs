use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub name: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}
