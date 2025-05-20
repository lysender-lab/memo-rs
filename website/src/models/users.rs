use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct NewUserFormData {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub role: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewUserData {
    pub username: String,
    pub password: String,
    pub status: String,
    pub roles: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserActiveFormData {
    pub token: String,
    pub active: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserStatusData {
    pub status: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRoleFormData {
    pub token: String,
    pub role: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserRolesData {
    pub roles: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResetPasswordFormData {
    pub token: String,
    pub password: String,
    pub confirm_password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResetPasswordData {
    pub password: String,
}
