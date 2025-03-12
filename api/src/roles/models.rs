use std::collections::HashSet;

use serde::Serialize;

#[derive(PartialEq, Debug, Clone, Serialize)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize)]
pub enum Permission {
    BucketsList,
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

impl TryFrom<&str> for Role {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Admin" => Ok(Role::Admin),
            "Editor" => Ok(Role::Editor),
            "Viewer" => Ok(Role::Viewer),
            _ => Err(format!(
                "Valid roles are: Admin, Editor, Viewer, got: {}",
                value
            )),
        }
    }
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Role::Admin => write!(f, "Admin"),
            Role::Editor => write!(f, "Editor"),
            Role::Viewer => write!(f, "Viewer"),
        }
    }
}

pub fn to_roles(list: Vec<String>) -> crate::Result<Vec<Role>> {
    let mut roles: Vec<Role> = Vec::new();
    for item in list.into_iter() {
        match Role::try_from(item.as_str()) {
            Ok(role) => roles.push(role),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(roles)
}

impl TryFrom<&str> for Permission {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "buckets.list" => Ok(Permission::BucketsList),
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
            _ => Err(format!("Invalid permission: {}", value)),
        }
    }
}

impl core::fmt::Display for Permission {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Permission::BucketsList => write!(f, "buckets.list"),
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

pub fn to_permissions(permissions: &Vec<String>) -> crate::Result<Vec<Permission>> {
    let mut perms: Vec<Permission> = Vec::new();
    for item in permissions.iter() {
        match Permission::try_from(item.as_str()) {
            Ok(permission) => perms.push(permission),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(perms)
}

/// Role to permissions mapping
pub fn role_permissions(role: &Role) -> Vec<Permission> {
    match role {
        Role::Admin => vec![
            Permission::BucketsList,
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
        Role::Editor => vec![
            Permission::BucketsList,
            Permission::BucketsView,
            Permission::DirsList,
            Permission::DirsView,
            Permission::FilesCreate,
            Permission::FilesList,
            Permission::FilesView,
        ],
        Role::Viewer => vec![
            Permission::BucketsList,
            Permission::BucketsView,
            Permission::DirsList,
            Permission::DirsView,
            Permission::FilesList,
            Permission::FilesView,
        ],
    }
}

/// Get all permissions for the given roles
pub fn roles_permissions(roles: &Vec<Role>) -> Vec<Permission> {
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
        let data = vec!["Admin".to_string(), "Viewer".to_string()];
        let roles = to_roles(data).unwrap();
        assert_eq!(roles, vec![Role::Admin, Role::Viewer]);
    }

    #[test]
    fn test_to_roles_invalid() {
        let data = vec!["Admin".to_string(), "InvalidRole".to_string()];
        let roles = to_roles(data);
        assert!(roles.is_err());
    }
}
