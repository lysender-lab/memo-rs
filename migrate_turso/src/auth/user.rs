use snafu::{OptionExt, ResultExt, ensure};
use validator::Validate;

use crate::error::{
    DbSnafu, InvalidRolesSnafu, MaxUsersReachedSnafu, ValidationSnafu, WhateverSnafu,
};
use crate::state::AppState;
use crate::{Error, Result};
use db::user::{
    ChangeCurrentPassword, MAX_USERS_PER_CLIENT, NewUser, UpdateUserPassword, UpdateUserRoles,
    UpdateUserStatus,
};
use memo::role::{Role, to_roles};
use memo::user::UserDto;
use memo::validators::flatten_errors;
use password::verify_password;

pub async fn create_user(
    state: &AppState,
    client_id: &str,
    data: &NewUser,
    is_setup: bool,
) -> Result<UserDto> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    let count = state
        .db
        .users
        .count_by_client(client_id)
        .await
        .context(DbSnafu)?;

    ensure!(count < MAX_USERS_PER_CLIENT as i64, MaxUsersReachedSnafu);

    // Username must be unique
    let existing = state
        .db
        .users
        .find_by_username(&data.username)
        .await
        .context(DbSnafu)?;

    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: "Username already exists".to_string(),
        }
    );

    // Roles must be all valid
    let roles: Vec<String> = data.roles.split(",").map(|item| item.to_string()).collect();
    // Validate roles
    let roles = to_roles(roles).context(InvalidRolesSnafu)?;

    // Should not allow creating a system admin
    if !is_setup {
        ensure!(
            !roles.contains(&Role::SystemAdmin),
            ValidationSnafu {
                msg: "Creating a system admin not allowed".to_string(),
            }
        );
    }

    state
        .db
        .users
        .create(client_id, data)
        .await
        .context(DbSnafu)
}

pub async fn update_user_status(
    state: &AppState,
    id: &str,
    data: &UpdateUserStatus,
) -> Result<bool> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    ensure!(
        &data.status == "active" || &data.status == "inactive",
        ValidationSnafu {
            msg: "User status must be active or inactive",
        }
    );

    state
        .db
        .users
        .update_status(id, data)
        .await
        .context(DbSnafu)
}

pub async fn update_user_roles(state: &AppState, id: &str, data: &UpdateUserRoles) -> Result<bool> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    // Roles must be all valid
    let roles_arr: Vec<String> = data.roles.split(",").map(|item| item.to_string()).collect();
    // Validate roles
    let roles_arr = to_roles(roles_arr).context(InvalidRolesSnafu)?;

    // Should not allow creating a system admin
    ensure!(
        !roles_arr.contains(&Role::SystemAdmin),
        ValidationSnafu {
            msg: "Creating a system admin not allowed".to_string(),
        }
    );

    state.db.users.update_roles(id, data).await.context(DbSnafu)
}

pub async fn change_current_password(
    state: &AppState,
    user_id: &str,
    data: &ChangeCurrentPassword,
) -> Result<bool> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    let passwd = state
        .db
        .users
        .get_password(user_id)
        .await
        .context(DbSnafu)?;

    let passwd = passwd.context(WhateverSnafu {
        msg: "Unable to re-query user".to_string(),
    })?;

    // Validate current password
    if let Err(verify_err) = verify_password(&data.current_password, &passwd) {
        return match verify_err {
            password::Error::InvalidPassword => Err(Error::Validation {
                msg: "Current password is incorrect".to_string(),
            }),
            _ => Err(format!("{}", verify_err).into()),
        };
    }

    let new_data = UpdateUserPassword {
        password: data.new_password.clone(),
    };

    state
        .db
        .users
        .update_password(user_id, &new_data)
        .await
        .context(DbSnafu)
}

pub async fn update_password(
    state: &AppState,
    user_id: &str,
    data: &UpdateUserPassword,
) -> Result<bool> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    state
        .db
        .users
        .update_password(user_id, data)
        .await
        .context(DbSnafu)
}
