use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    roles::{Permission, Role, roles_permissions, to_permissions},
    users::UserDto,
};

#[derive(Clone)]
pub struct ActorPayload {
    pub id: String,
    pub client_id: String,
    pub default_bucket_id: Option<String>,
    pub scope: String,
}

#[derive(Clone, Serialize)]
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
    pub fn new(payload: ActorPayload, user: UserDto) -> Self {
        let roles = user.roles.clone();
        let permissions: Vec<Permission> = roles_permissions(&roles).into_iter().collect();
        // Convert to string to allow sorting
        let mut permissions: Vec<String> = permissions.iter().map(|p| p.to_string()).collect();
        permissions.sort();
        // Convert again to Permission enum
        let permissions: Vec<Permission> =
            to_permissions(&permissions).expect("Invalid permissions");

        Actor {
            id: user.id.clone(),
            client_id: payload.client_id,
            default_bucket_id: payload.default_bucket_id,
            scope: payload.scope,
            user,
            roles,
            permissions,
        }
    }

    pub fn has_auth_scope(&self) -> bool {
        self.has_scope("auth")
    }

    pub fn has_files_scope(&self) -> bool {
        self.has_scope("files")
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scope.contains(scope)
    }

    pub fn has_permissions(&self, permissions: &Vec<Permission>) -> bool {
        permissions
            .iter()
            .all(|permission| self.permissions.contains(permission))
    }
}

#[derive(Deserialize, Serialize, Validate)]
pub struct Credentials {
    #[validate(length(min = 1, max = 30))]
    pub username: String,

    #[validate(length(min = 8, max = 100))]
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthToken {
    pub token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user: UserDto,
    pub token: String,
}
