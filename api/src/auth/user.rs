use async_trait::async_trait;

use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, ensure};
use validator::Validate;

use super::password::hash_password;
use crate::auth::password::verify_password;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, InvalidRolesSnafu, MaxUsersReachedSnafu,
    ValidationSnafu, WhateverSnafu,
};
use crate::schema::users::{self, dsl};
use crate::state::AppState;
use crate::{Error, Result};
use memo::role::{Role, to_roles};
use memo::user::UserDto;
use memo::utils::generate_id;
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub status: String,
    pub roles: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        let role_list = user.roles.split(",").map(|item| item.to_string()).collect();
        let roles = to_roles(role_list).expect("Invalid roles");
        UserDto {
            id: user.id,
            client_id: user.client_id,
            username: user.username,
            status: user.status,
            roles,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewUser {
    #[validate(length(min = 1, max = 30))]
    #[validate(custom(function = "memo::validators::alphanumeric"))]
    pub username: String,

    #[validate(length(min = 8, max = 60))]
    pub password: String,

    #[validate(length(min = 1, max = 100))]
    #[validate(custom(function = "memo::validators::csvname"))]
    pub roles: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserStatus {
    #[validate(length(min = 1, max = 10))]
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserRoles {
    #[validate(length(min = 1, max = 100))]
    #[validate(custom(function = "memo::validators::csvname"))]
    pub roles: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserPassword {
    #[validate(length(min = 8, max = 60))]
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ChangeCurrentPassword {
    #[validate(length(min = 8, max = 60))]
    pub current_password: String,

    #[validate(length(min = 8, max = 60))]
    pub new_password: String,
}

const MAX_USERS_PER_CLIENT: i32 = 50;

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

    let user = state.db.users.get(user_id).await?.context(WhateverSnafu {
        msg: "Unable to re-query user".to_string(),
    })?;

    // Validate current password
    if let Err(verify_err) = verify_password(&data.current_password, &user.password) {
        return match verify_err {
            Error::InvalidPassword => Err(Error::Validation {
                msg: "Current password is incorrect".to_string(),
            }),
            _ => Err(verify_err),
        };
    }

    let new_data = UpdateUserPassword {
        password: data.new_password.clone(),
    };

    state.db.users.update_password(user_id, &new_data).await
}

#[async_trait]
pub trait UserRepoable: Send + Sync {
    async fn list(&self, client_id: &str) -> Result<Vec<User>>;

    async fn create(&self, client_id: &str, data: &NewUser, is_setup: bool) -> Result<User>;

    async fn get(&self, id: &str) -> Result<Option<User>>;

    async fn find_by_username(&self, username: &str) -> Result<Option<User>>;

    async fn count_by_client(&self, client_id: &str) -> Result<i64>;

    async fn update_status(&self, id: &str, data: &UpdateUserStatus) -> Result<bool>;

    async fn update_roles(&self, id: &str, data: &UpdateUserRoles) -> Result<bool>;

    async fn update_password(&self, id: &str, data: &UpdateUserPassword) -> Result<bool>;

    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct UserRepo {
    db_pool: Pool,
}

impl UserRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl UserRepoable for UserRepo {
    async fn list(&self, client_id: &str) -> Result<Vec<User>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let client_id = client_id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::users
                    .filter(dsl::client_id.eq(&client_id))
                    .select(User::as_select())
                    .order(dsl::username.asc())
                    .load::<User>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(items)
    }

    async fn create(&self, client_id: &str, data: &NewUser, is_setup: bool) -> Result<User> {
        let errors = data.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;
        let count = self.count_by_client(client_id).await?;
        ensure!(count < MAX_USERS_PER_CLIENT as i64, MaxUsersReachedSnafu);

        // Username must be unique
        let existing = self.find_by_username(&data.username).await?;
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

        let data_copy = data.clone();
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password)?;

        let dir = User {
            id: generate_id(),
            client_id: client_id.to_string(),
            username: data_copy.username,
            password: hashed,
            status: "active".to_string(),
            roles: data_copy.roles,
            created_at: today,
            updated_at: today,
        };

        let user_copy = dir.clone();
        let inser_res = db
            .interact(move |conn| {
                diesel::insert_into(users::table)
                    .values(&user_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = inser_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(dir)
    }

    async fn get(&self, id: &str) -> Result<Option<User>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::users
                    .find(&id)
                    .select(User::as_select())
                    .first::<User>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let user = select_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(user)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let username = username.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::users
                    .filter(dsl::username.eq(&username))
                    .select(User::as_select())
                    .first::<User>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let user = select_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(user)
    }

    async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let client_id = client_id.to_string();
        let count_res = db
            .interact(move |conn| {
                dsl::users
                    .filter(dsl::client_id.eq(&client_id))
                    .select(count_star())
                    .get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(count)
    }

    async fn update_status(&self, id: &str, data: &UpdateUserStatus) -> Result<bool> {
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

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let status = data.status.clone();
        let today = chrono::Utc::now().timestamp();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::users)
                    .filter(dsl::id.eq(&id))
                    .set((dsl::status.eq(&status), dsl::updated_at.eq(today)))
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let affected = update_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(affected > 0)
    }

    async fn update_roles(&self, id: &str, data: &UpdateUserRoles) -> Result<bool> {
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

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let roles = data.roles.clone();
        let today = chrono::Utc::now().timestamp();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::users)
                    .filter(dsl::id.eq(&id))
                    .set((dsl::roles.eq(&roles), dsl::updated_at.eq(today)))
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let affected = update_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(affected > 0)
    }

    async fn update_password(&self, id: &str, data: &UpdateUserPassword) -> Result<bool> {
        let errors = data.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password)?;
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::users)
                    .filter(dsl::id.eq(&id))
                    .set((dsl::password.eq(&hashed), dsl::updated_at.eq(today)))
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let affected = update_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(affected > 0)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // It is okay to delete user even if there are potential references
        // to created buckets, dirs or files
        let id = id.to_string();
        let delete_res = db
            .interact(move |conn| diesel::delete(dsl::users.filter(dsl::id.eq(&id))).execute(conn))
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "users".to_string(),
        })?;

        Ok(())
    }
}

#[cfg(test)]
pub const TEST_ADMIN_USER_ID: &'static str = "0196d1ace11e715bbc32fd4e88226f56";

#[cfg(test)]
pub const TEST_USER_ID: &'static str = "0196d1adc6807c2c8aa49982466faf88";

#[cfg(test)]
pub fn create_test_admin_user() -> Result<User> {
    use crate::client::TEST_ADMIN_CLIENT_ID;

    let password = hash_password("secret-password")?;
    let today = chrono::Utc::now().timestamp();

    Ok(User {
        id: TEST_ADMIN_USER_ID.to_string(),
        client_id: TEST_ADMIN_CLIENT_ID.to_string(),
        username: "admin".to_string(),
        password,
        status: "active".to_string(),
        roles: "SystemAdmin".to_string(),
        created_at: today.clone(),
        updated_at: today,
    })
}

#[cfg(test)]
pub fn create_test_user() -> Result<User> {
    use crate::client::TEST_CLIENT_ID;

    let password = hash_password("secret-password")?;
    let today = chrono::Utc::now().timestamp();

    Ok(User {
        id: TEST_USER_ID.to_string(),
        client_id: TEST_CLIENT_ID.to_string(),
        username: "user".to_string(),
        password,
        status: "active".to_string(),
        roles: "Admin".to_string(),
        created_at: today.clone(),
        updated_at: today,
    })
}

#[cfg(test)]
pub struct UserTestRepo {}

#[cfg(test)]
#[async_trait]
impl UserRepoable for UserTestRepo {
    async fn list(&self, client_id: &str) -> Result<Vec<User>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let filtered: Vec<User> = users
            .into_iter()
            .filter(|x| x.client_id.as_str() == client_id)
            .collect();
        Ok(filtered)
    }

    async fn create(&self, _client_id: &str, _data: &NewUser, _is_setup: bool) -> Result<User> {
        Err("Not supported".into())
    }

    async fn get(&self, id: &str) -> Result<Option<User>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let found = users.into_iter().find(|x| x.id.as_str() == id);
        Ok(found)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let found = users.into_iter().find(|x| x.username.as_str() == username);
        Ok(found)
    }

    async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let users = self.list(client_id).await?;
        Ok(users.len() as i64)
    }

    async fn update_status(&self, _id: &str, _data: &UpdateUserStatus) -> Result<bool> {
        Ok(true)
    }

    async fn update_roles(&self, _id: &str, _data: &UpdateUserRoles) -> Result<bool> {
        Ok(true)
    }

    async fn update_password(&self, _id: &str, _data: &UpdateUserPassword) -> Result<bool> {
        Ok(true)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
