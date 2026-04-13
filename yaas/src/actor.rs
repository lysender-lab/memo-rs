use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::role::{Permission, Role, Scope, roles_permissions, to_permissions};
use crate::user::UserDto;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActorDto {
    pub id: String,
    pub org_id: String,
    pub org_count: i32,
    pub scopes: Vec<Scope>,
    pub user: UserDto,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

#[derive(Clone)]
pub struct ActorPayloadDto {
    pub id: String,
    pub org_id: String,
    pub org_count: i32,
    pub roles: Vec<Role>,
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Actor {
    pub actor: Option<ActorDto>,
}

impl Actor {
    pub fn new(payload: ActorPayloadDto, user: UserDto) -> Self {
        let permissions: Vec<Permission> = roles_permissions(&payload.roles).into_iter().collect();

        // Convert to string to allow sorting
        let mut permissions: Vec<String> = permissions.iter().map(|p| p.to_string()).collect();
        permissions.sort();

        // Convert again to Permission enum
        let permissions: Vec<Permission> =
            to_permissions(&permissions).expect("Permissions should convert back to enum");

        Actor {
            actor: Some(ActorDto {
                id: payload.id,
                org_id: payload.org_id,
                org_count: payload.org_count,
                scopes: payload.scopes,
                user,
                roles: payload.roles,
                permissions,
            }),
        }
    }

    pub fn has_auth_scope(&self) -> bool {
        self.has_scope(Scope::Auth)
    }

    pub fn has_vault_scope(&self) -> bool {
        self.has_scope(Scope::Vault)
    }

    pub fn has_scope(&self, scope: Scope) -> bool {
        match &self.actor {
            Some(actor) => actor.scopes.contains(&scope),
            None => false,
        }
    }

    pub fn has_permissions(&self, permissions: &[Permission]) -> bool {
        match &self.actor {
            Some(actor) => permissions
                .iter()
                .all(|permission| actor.permissions.contains(permission)),
            None => false,
        }
    }

    pub fn is_system_admin(&self) -> bool {
        match &self.actor {
            Some(actor) => actor.roles.contains(&Role::Superuser),
            None => false,
        }
    }

    pub fn member_of(&self, org_id: &str) -> bool {
        match &self.actor {
            Some(actor) => actor.org_id == org_id,
            None => false,
        }
    }
}

impl Default for Actor {
    /// Empty actor for unauthenticated requests
    fn default() -> Self {
        Actor { actor: None }
    }
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CredentialsDto {
    #[validate(length(max = 100))]
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 8, max = 60))]
    pub password: String,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct SwitchAuthContextDto {
    pub org_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthResponseDto {
    pub user: UserDto,
    pub token: String,
    pub org_id: String,
    pub org_count: i32,
}
