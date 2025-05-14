use async_trait::async_trait;

use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use validator::Validate;

use super::password::hash_password;
use crate::Result;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, InvalidRolesSnafu, MaxUsersReachedSnafu,
    ValidationSnafu,
};
use crate::schema::users::{self, dsl};
use memo::dto::user::UserDto;
use memo::role::to_roles;
use memo::utils::generate_id;
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
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

    #[validate(length(min = 8, max = 100))]
    pub password: String,

    #[validate(length(min = 1, max = 100))]
    #[validate(custom(function = "memo::validators::csvname"))]
    pub roles: String,
}

const MAX_USERS_PER_CLIENT: i32 = 50;

#[async_trait]
pub trait UserRepoable: Send + Sync {
    async fn list(&self, client_id: &str) -> Result<Vec<User>>;

    async fn create(&self, client_id: &str, data: &NewUser) -> Result<User>;

    async fn get(&self, id: &str) -> Result<Option<User>>;

    async fn find_by_username(&self, username: &str) -> Result<Option<User>>;

    async fn count_by_client(&self, client_id: &str) -> Result<i64>;

    async fn update_status(&self, id: &str, status: &str) -> Result<bool>;

    async fn update_password(&self, id: &str, password: &str) -> Result<bool>;

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

    async fn create(&self, client_id: &str, data: &NewUser) -> Result<User> {
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
        let _ = to_roles(roles).context(InvalidRolesSnafu)?;

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

    async fn update_status(&self, id: &str, status: &str) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let status = status.to_string();
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

    async fn update_password(&self, id: &str, password: &str) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&password)?;
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
