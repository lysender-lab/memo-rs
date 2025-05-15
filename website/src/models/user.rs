use memo::dto::user::UserDto;
use memo::role::{Permission, Role};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub client_id: String,
    pub default_bucket_id: Option<String>,
    pub scope: String,
    pub user: UserDto,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

impl Actor {
    pub fn has_permissions(&self, permissions: &Vec<Permission>) -> bool {
        permissions
            .iter()
            .all(|perm| self.permissions.contains(perm))
    }
}
