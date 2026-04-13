use serde::{Deserialize, Deserializer, Serialize, Serializer};
use snafu::ensure;
use std::collections::HashSet;

use crate::Result;
use crate::error::{InvalidPermissionsSnafu, InvalidRolesSnafu, InvalidScopesSnafu};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    Superuser,
    OrgAdmin,
    OrgEditor,
    OrgViewer,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Permission {
    UsersCreate,
    UsersEdit,
    UsersDelete,
    UsersList,
    UsersView,
    UsersManage,

    AppsCreate,
    AppsEdit,
    AppsDelete,
    AppsList,
    AppsView,
    AppsManage,

    OrgsCreate,
    OrgsEdit,
    OrgsDelete,
    OrgsList,
    OrgsView,
    OrgsManage,

    OrgMembersCreate,
    OrgMembersEdit,
    OrgMembersDelete,
    OrgMembersList,
    OrgMembersView,
    OrgMembersManage,

    OrgAppsCreate,
    OrgAppsEdit,
    OrgAppsDelete,
    OrgAppsList,
    OrgAppsView,
    OrgAppsManage,

    BucketsEdit,
    BucketsView,

    DirsCreate,
    DirsEdit,
    DirsDelete,
    DirsList,
    DirsView,
    DirsManage,

    FilesCreate,
    FilesEdit,
    FilesDelete,
    FilesList,
    FilesView,
    FilesManage,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Scope {
    Auth,
    Vault,
    Oauth,
}

impl TryFrom<&str> for Role {
    type Error = String;

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "Superuser" => Ok(Role::Superuser),
            "OrgAdmin" => Ok(Role::OrgAdmin),
            "OrgEditor" => Ok(Role::OrgEditor),
            "OrgViewer" => Ok(Role::OrgViewer),
            _ => Err(format!("Invalid role: {value}")),
        }
    }
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Role::Superuser => write!(f, "Superuser"),
            Role::OrgAdmin => write!(f, "OrgAdmin"),
            Role::OrgEditor => write!(f, "OrgEditor"),
            Role::OrgViewer => write!(f, "OrgViewer"),
        }
    }
}

pub fn to_roles(list: &[String]) -> Result<Vec<Role>> {
    let mut roles: Vec<Role> = Vec::with_capacity(list.len());
    let mut errors: Vec<String> = Vec::with_capacity(list.len());
    for item in list.iter() {
        let role = item.as_str();
        match Role::try_from(role) {
            Ok(role) => roles.push(role),
            Err(_) => errors.push(role.to_string()),
        }
    }

    ensure!(
        errors.is_empty(),
        InvalidRolesSnafu {
            msg: errors.join(", ")
        }
    );

    Ok(roles)
}

impl TryFrom<&str> for Scope {
    type Error = String;

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "auth" => Ok(Scope::Auth),
            "vault" => Ok(Scope::Vault),
            "oauth" => Ok(Scope::Oauth),
            _ => Err(format!("Invalid scope: {value}")),
        }
    }
}

impl TryFrom<String> for Scope {
    type Error = String;

    fn try_from(value: String) -> core::result::Result<Self, Self::Error> {
        Scope::try_from(value.as_str())
    }
}

impl core::fmt::Display for Scope {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Scope::Auth => write!(f, "auth"),
            Scope::Vault => write!(f, "vault"),
            Scope::Oauth => write!(f, "oauth"),
        }
    }
}

impl Serialize for Scope {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Scope {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Scope::try_from(value.as_str()).map_err(serde::de::Error::custom)
    }
}

pub fn to_scopes(list: &[String]) -> Result<Vec<Scope>> {
    let mut scopes: Vec<Scope> = Vec::with_capacity(list.len());
    let mut errors: Vec<String> = Vec::with_capacity(list.len());
    for item in list.iter() {
        let scope = item.as_str();
        match Scope::try_from(scope) {
            Ok(scope) => scopes.push(scope),
            Err(_) => errors.push(scope.to_string()),
        }
    }

    ensure!(
        errors.is_empty(),
        InvalidScopesSnafu {
            msg: errors.join(", ")
        }
    );

    Ok(scopes)
}

impl TryFrom<&str> for Permission {
    type Error = String;

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "users.create" => Ok(Permission::UsersCreate),
            "users.edit" => Ok(Permission::UsersEdit),
            "users.delete" => Ok(Permission::UsersDelete),
            "users.list" => Ok(Permission::UsersList),
            "users.view" => Ok(Permission::UsersView),
            "users.manage" => Ok(Permission::UsersManage),

            "apps.create" => Ok(Permission::AppsCreate),
            "apps.edit" => Ok(Permission::AppsEdit),
            "apps.delete" => Ok(Permission::AppsDelete),
            "apps.list" => Ok(Permission::AppsList),
            "apps.view" => Ok(Permission::AppsView),
            "apps.manage" => Ok(Permission::AppsManage),

            "orgs.create" => Ok(Permission::OrgsCreate),
            "orgs.edit" => Ok(Permission::OrgsEdit),
            "orgs.delete" => Ok(Permission::OrgsDelete),
            "orgs.list" => Ok(Permission::OrgsList),
            "orgs.view" => Ok(Permission::OrgsView),
            "orgs.manage" => Ok(Permission::OrgsManage),

            "org_members.create" => Ok(Permission::OrgMembersCreate),
            "org_members.edit" => Ok(Permission::OrgMembersEdit),
            "org_members.delete" => Ok(Permission::OrgMembersDelete),
            "org_members.list" => Ok(Permission::OrgMembersList),
            "org_members.view" => Ok(Permission::OrgMembersView),
            "org_members.manage" => Ok(Permission::OrgMembersManage),

            "org_apps.create" => Ok(Permission::OrgAppsCreate),
            "org_apps.edit" => Ok(Permission::OrgAppsEdit),
            "org_apps.delete" => Ok(Permission::OrgAppsDelete),
            "org_apps.list" => Ok(Permission::OrgAppsList),
            "org_apps.view" => Ok(Permission::OrgAppsView),
            "org_apps.manage" => Ok(Permission::OrgAppsManage),

            "buckets.edit" => Ok(Permission::BucketsEdit),
            "buckets.view" => Ok(Permission::BucketsView),

            "dirs.create" => Ok(Permission::DirsCreate),
            "dirs.edit" => Ok(Permission::DirsEdit),
            "dirs.delete" => Ok(Permission::DirsDelete),
            "dirs.list" => Ok(Permission::DirsList),
            "dirs.view" => Ok(Permission::DirsView),
            "dirs.manage" => Ok(Permission::DirsManage),

            "files.create" => Ok(Permission::FilesCreate),
            "files.edit" => Ok(Permission::FilesEdit),
            "files.delete" => Ok(Permission::FilesDelete),
            "files.list" => Ok(Permission::FilesList),
            "files.view" => Ok(Permission::FilesView),
            "files.manage" => Ok(Permission::FilesManage),

            _ => Err(format!("Invalid permission: {value}")),
        }
    }
}

impl TryFrom<String> for Permission {
    type Error = String;

    fn try_from(value: String) -> core::result::Result<Self, Self::Error> {
        Permission::try_from(value.as_str())
    }
}

impl core::fmt::Display for Permission {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Permission::UsersCreate => write!(f, "users.create"),
            Permission::UsersEdit => write!(f, "users.edit"),
            Permission::UsersDelete => write!(f, "users.delete"),
            Permission::UsersList => write!(f, "users.list"),
            Permission::UsersView => write!(f, "users.view"),
            Permission::UsersManage => write!(f, "users.manage"),

            Permission::AppsCreate => write!(f, "apps.create"),
            Permission::AppsEdit => write!(f, "apps.edit"),
            Permission::AppsDelete => write!(f, "apps.delete"),
            Permission::AppsList => write!(f, "apps.list"),
            Permission::AppsView => write!(f, "apps.view"),
            Permission::AppsManage => write!(f, "apps.manage"),

            Permission::OrgsCreate => write!(f, "orgs.create"),
            Permission::OrgsEdit => write!(f, "orgs.edit"),
            Permission::OrgsDelete => write!(f, "orgs.delete"),
            Permission::OrgsList => write!(f, "orgs.list"),
            Permission::OrgsView => write!(f, "orgs.view"),
            Permission::OrgsManage => write!(f, "orgs.manage"),

            Permission::OrgMembersCreate => write!(f, "org_members.create"),
            Permission::OrgMembersEdit => write!(f, "org_members.edit"),
            Permission::OrgMembersDelete => write!(f, "org_members.delete"),
            Permission::OrgMembersList => write!(f, "org_members.list"),
            Permission::OrgMembersView => write!(f, "org_members.view"),
            Permission::OrgMembersManage => write!(f, "org_members.manage"),

            Permission::OrgAppsCreate => write!(f, "org_apps.create"),
            Permission::OrgAppsEdit => write!(f, "org_apps.edit"),
            Permission::OrgAppsDelete => write!(f, "org_apps.delete"),
            Permission::OrgAppsList => write!(f, "org_apps.list"),
            Permission::OrgAppsView => write!(f, "org_apps.view"),
            Permission::OrgAppsManage => write!(f, "org_apps.manage"),

            Permission::BucketsEdit => write!(f, "buckets.edit"),
            Permission::BucketsView => write!(f, "buckets.view"),

            Permission::DirsCreate => write!(f, "dirs.create"),
            Permission::DirsEdit => write!(f, "dirs.edit"),
            Permission::DirsDelete => write!(f, "dirs.delete"),
            Permission::DirsList => write!(f, "dirs.list"),
            Permission::DirsView => write!(f, "dirs.view"),
            Permission::DirsManage => write!(f, "dirs.manage"),

            Permission::FilesCreate => write!(f, "files.create"),
            Permission::FilesEdit => write!(f, "files.edit"),
            Permission::FilesDelete => write!(f, "files.delete"),
            Permission::FilesList => write!(f, "files.list"),
            Permission::FilesView => write!(f, "files.view"),
            Permission::FilesManage => write!(f, "files.manage"),
        }
    }
}

impl Serialize for Permission {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Permission::try_from(value.as_str()).map_err(serde::de::Error::custom)
    }
}

pub fn to_permissions(permissions: &[String]) -> Result<Vec<Permission>> {
    let mut perms: Vec<Permission> = Vec::with_capacity(permissions.len());
    let mut errors: Vec<String> = Vec::with_capacity(permissions.len());
    for item in permissions.iter() {
        let perm = item.as_str();
        match Permission::try_from(perm) {
            Ok(permission) => perms.push(permission),
            Err(_) => errors.push(perm.to_string()),
        }
    }

    ensure!(
        errors.is_empty(),
        InvalidPermissionsSnafu {
            msg: errors.join(", ")
        }
    );

    Ok(perms)
}

/// Role to permissions mapping
pub fn role_permissions(role: &Role) -> Vec<Permission> {
    match role {
        Role::Superuser => vec![
            // Superuser should only be able to work inside yaas
            Permission::UsersCreate,
            Permission::UsersEdit,
            Permission::UsersDelete,
            Permission::UsersList,
            Permission::UsersView,
            Permission::UsersManage,
            Permission::AppsCreate,
            Permission::AppsEdit,
            Permission::AppsDelete,
            Permission::AppsList,
            Permission::AppsView,
            Permission::AppsManage,
            Permission::OrgsCreate,
            Permission::OrgsEdit,
            Permission::OrgsDelete,
            Permission::OrgsList,
            Permission::OrgsView,
            Permission::OrgsManage,
            Permission::OrgMembersCreate,
            Permission::OrgMembersEdit,
            Permission::OrgMembersDelete,
            Permission::OrgMembersList,
            Permission::OrgMembersView,
            Permission::OrgMembersManage,
            Permission::OrgAppsCreate,
            Permission::OrgAppsEdit,
            Permission::OrgAppsDelete,
            Permission::OrgAppsList,
            Permission::OrgAppsView,
            Permission::OrgAppsManage,
        ],
        Role::OrgAdmin => vec![
            Permission::UsersList,
            Permission::UsersView,
            Permission::OrgsEdit,
            Permission::OrgsDelete,
            Permission::OrgsList,
            Permission::OrgsView,
            Permission::OrgsManage,
            Permission::OrgMembersCreate,
            Permission::OrgMembersEdit,
            Permission::OrgMembersDelete,
            Permission::OrgMembersList,
            Permission::OrgMembersView,
            Permission::OrgMembersManage,
            Permission::OrgAppsList,
            Permission::OrgAppsView,
            Permission::BucketsEdit,
            Permission::BucketsView,
            Permission::DirsCreate,
            Permission::DirsEdit,
            Permission::DirsDelete,
            Permission::DirsList,
            Permission::DirsView,
            Permission::DirsManage,
            Permission::FilesCreate,
            Permission::FilesEdit,
            Permission::FilesDelete,
            Permission::FilesList,
            Permission::FilesView,
            Permission::FilesManage,
        ],
        Role::OrgEditor => vec![
            Permission::UsersList,
            Permission::UsersView,
            Permission::OrgsList,
            Permission::OrgsView,
            Permission::OrgMembersList,
            Permission::OrgMembersView,
            Permission::OrgAppsList,
            Permission::OrgAppsView,
            Permission::BucketsView,
            Permission::DirsCreate,
            Permission::DirsEdit,
            Permission::DirsDelete,
            Permission::DirsList,
            Permission::DirsView,
            Permission::FilesCreate,
            Permission::FilesEdit,
            Permission::FilesDelete,
            Permission::FilesList,
            Permission::FilesView,
        ],
        Role::OrgViewer => vec![
            Permission::UsersView,
            Permission::OrgsList,
            Permission::OrgsView,
            Permission::OrgMembersList,
            Permission::OrgMembersView,
            Permission::OrgAppsList,
            Permission::OrgAppsView,
            Permission::BucketsView,
            Permission::DirsList,
            Permission::DirsView,
            Permission::FilesList,
            Permission::FilesView,
        ],
    }
}

/// Get all permissions for the given roles
pub fn roles_permissions(roles: &[Role]) -> Vec<Permission> {
    let mut permissions: HashSet<Permission> = HashSet::new();
    roles.iter().for_each(|role| {
        role_permissions(role).iter().for_each(|p| {
            permissions.insert(p.clone());
        });
    });
    permissions.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_roles_valid() {
        let data = vec!["OrgAdmin".to_string(), "OrgViewer".to_string()];
        let roles = to_roles(&data).unwrap();
        assert_eq!(roles, vec![Role::OrgAdmin, Role::OrgViewer]);
    }

    #[test]
    fn test_to_roles_invalid() {
        let data = vec![
            "OrgAdmin".to_string(),
            "InvalidRole".to_string(),
            "NetflixRole".to_string(),
        ];
        let roles = to_roles(&data);
        assert!(roles.is_err());
        if let Err(e) = roles {
            assert_eq!(e.to_string(), "Invalid roles: InvalidRole, NetflixRole");
        }
    }

    #[test]
    fn test_to_permissions_valid() {
        let data = vec![
            "orgs.create".to_string(),
            "orgs.edit".to_string(),
            "orgs.delete".to_string(),
        ];
        let permissions = to_permissions(&data).unwrap();
        assert_eq!(
            permissions,
            vec![
                Permission::OrgsCreate,
                Permission::OrgsEdit,
                Permission::OrgsDelete,
            ]
        );
    }

    #[test]
    fn test_to_permissions_invalid() {
        let data = vec![
            "orgs.create".to_string(),
            "orgs.edit".to_string(),
            "orgs.delete".to_string(),
            "netflix.binge".to_string(),
            "netflix.watch".to_string(),
        ];
        let permissions = to_permissions(&data);
        assert!(permissions.is_err());
        if let Err(e) = permissions {
            assert_eq!(
                e.to_string(),
                "Invalid permissions: netflix.binge, netflix.watch"
            );
        }
    }

    #[test]
    fn test_to_scopes_valid() {
        let data = vec!["auth".to_string(), "vault".to_string()];
        let scopes = to_scopes(&data).unwrap();
        assert_eq!(scopes, vec![Scope::Auth, Scope::Vault]);
    }

    #[test]
    fn test_to_scopes_invalid() {
        let data = vec!["auth".to_string(), "Netflix".to_string()];
        let scopes = to_scopes(&data);
        assert!(scopes.is_err());
        if let Err(e) = scopes {
            assert_eq!(e.to_string(), "Invalid scopes: Netflix");
        }
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(Scope::Auth.to_string(), "auth");
        assert_eq!(Scope::Vault.to_string(), "vault");
        assert_eq!(Scope::Oauth.to_string(), "oauth");
    }

    #[test]
    fn test_scope_serde_uses_display_format() {
        let json = serde_json::to_string(&Scope::Auth).unwrap();
        assert_eq!(json, "\"auth\"");

        let scope: Scope = serde_json::from_str("\"vault\"").unwrap();
        assert_eq!(scope, Scope::Vault);
    }

    #[test]
    fn test_permission_serde_uses_display_format() {
        let json = serde_json::to_string(&Permission::UsersCreate).unwrap();
        assert_eq!(json, "\"users.create\"");

        let permission: Permission = serde_json::from_str("\"org_members.view\"").unwrap();
        assert_eq!(permission, Permission::OrgMembersView);
    }
}
