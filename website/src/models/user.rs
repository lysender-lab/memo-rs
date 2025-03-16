use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub client_id: String,
    pub username: String,
    pub status: String,
    pub roles: Vec<Role>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub client_id: String,
    pub default_bucket_id: Option<String>,
    pub scope: String,
    pub user: User,
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

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

#[derive(PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
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
