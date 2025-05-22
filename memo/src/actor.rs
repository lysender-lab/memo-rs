use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::role::{Permission, Role, roles_permissions, to_permissions};
use crate::user::UserDto;

#[derive(Clone)]
pub struct ActorPayload {
    pub id: String,
    pub client_id: String,
    pub default_bucket_id: Option<String>,
    pub scope: String,
}

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

    /// Empty actor for unauthenticated requests
    pub fn empty() -> Self {
        Actor {
            id: "unknown".to_string(),
            client_id: "unknown".to_string(),
            default_bucket_id: None,
            scope: "".to_string(),
            user: UserDto {
                id: "unknown".to_string(),
                client_id: "unknown".to_string(),
                username: "unknown".to_string(),
                status: "unknown".to_string(),
                roles: vec![],
                created_at: 0,
                updated_at: 0,
            },
            roles: vec![],
            permissions: vec![],
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

    pub fn is_system_admin(&self) -> bool {
        self.user
            .roles
            .iter()
            .find(|role| **role == Role::SystemAdmin)
            .is_some()
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

#[cfg(test)]
mod tests {
    use crate::utils::generate_id;

    use super::*;

    #[test]
    fn test_empty_actor() {
        let actor = Actor::empty();
        assert_eq!(actor.has_auth_scope(), false);
        assert_eq!(actor.is_system_admin(), false);
    }

    #[test]
    fn test_regular_actor() {
        let client_id = generate_id();
        let actor = Actor::new(
            ActorPayload {
                id: generate_id(),
                client_id: client_id.clone(),
                default_bucket_id: None,
                scope: "auth".to_string(),
            },
            UserDto {
                id: generate_id(),
                client_id,
                username: "test".to_string(),
                status: "active".to_string(),
                roles: vec![Role::Admin],
                created_at: 0,
                updated_at: 0,
            },
        );
        assert_eq!(actor.has_auth_scope(), true);
        assert_eq!(actor.is_system_admin(), false);
    }

    #[test]
    fn test_system_admin_actor() {
        let client_id = generate_id();
        let actor = Actor::new(
            ActorPayload {
                id: generate_id(),
                client_id: client_id.clone(),
                default_bucket_id: None,
                scope: "auth".to_string(),
            },
            UserDto {
                id: generate_id(),
                client_id,
                username: "test".to_string(),
                status: "active".to_string(),
                roles: vec![Role::SystemAdmin],
                created_at: 0,
                updated_at: 0,
            },
        );
        assert_eq!(actor.has_auth_scope(), true);
        assert_eq!(actor.is_system_admin(), true);
    }
}
