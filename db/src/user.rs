use async_trait::async_trait;

use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use serde::Deserialize;
use snafu::ResultExt;
use validator::Validate;

use crate::Result;
use crate::error::{DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, HashPasswordSnafu};
use crate::schema::users::{self, dsl};
use memo::role::to_roles;
use memo::user::UserDto;
use memo::utils::generate_id;
use password::hash_password;

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

pub const MAX_USERS_PER_CLIENT: i32 = 50;

#[async_trait]
pub trait UserStore: Send + Sync {
    async fn list(&self, client_id: &str) -> Result<Vec<UserDto>>;

    async fn create(&self, client_id: &str, data: &NewUser, is_setup: bool) -> Result<UserDto>;

    async fn get(&self, id: &str) -> Result<Option<UserDto>>;

    async fn get_password(&self, id: &str) -> Result<Option<String>>;

    async fn find_by_username(&self, username: &str) -> Result<Option<UserDto>>;

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
impl UserStore for UserRepo {
    async fn list(&self, client_id: &str) -> Result<Vec<UserDto>> {
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

        let items: Vec<UserDto> = items.into_iter().map(|x| x.into()).collect();

        Ok(items)
    }

    async fn create(&self, client_id: &str, data: &NewUser, is_setup: bool) -> Result<UserDto> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let data_copy = data.clone();
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password).context(HashPasswordSnafu)?;

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

        Ok(dir.into())
    }

    async fn get(&self, id: &str) -> Result<Option<UserDto>> {
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

        Ok(user.map(|x| x.into()))
    }

    async fn get_password(&self, id: &str) -> Result<Option<String>> {
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

        Ok(user.map(|x| x.password))
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<UserDto>> {
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

        Ok(user.map(|x| x.into()))
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
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password).context(HashPasswordSnafu)?;
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

#[cfg(feature = "test")]
pub const TEST_ADMIN_USER_ID: &'static str = "0196d1ace11e715bbc32fd4e88226f56";

#[cfg(feature = "test")]
pub const TEST_USER_ID: &'static str = "0196d1adc6807c2c8aa49982466faf88";

#[cfg(feature = "test")]
pub fn create_test_admin_user() -> Result<User> {
    use crate::client::TEST_ADMIN_CLIENT_ID;

    let password = hash_password("secret-password").context(HashPasswordSnafu)?;
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

#[cfg(feature = "test")]
pub fn create_test_user() -> Result<User> {
    use crate::client::TEST_CLIENT_ID;

    let password = hash_password("secret-password").context(HashPasswordSnafu)?;
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

#[cfg(feature = "test")]
pub struct UserTestRepo {}

#[cfg(feature = "test")]
#[async_trait]
impl UserStore for UserTestRepo {
    async fn list(&self, client_id: &str) -> Result<Vec<UserDto>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let filtered: Vec<UserDto> = users
            .into_iter()
            .filter(|x| x.client_id.as_str() == client_id)
            .map(|x| x.into())
            .collect();
        Ok(filtered)
    }

    async fn create(&self, _client_id: &str, _data: &NewUser, _is_setup: bool) -> Result<UserDto> {
        Err("Not supported".into())
    }

    async fn get(&self, id: &str) -> Result<Option<UserDto>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let found = users.into_iter().find(|x| x.id.as_str() == id);
        Ok(found.map(|x| x.into()))
    }

    async fn get_password(&self, id: &str) -> Result<Option<String>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let found = users.into_iter().find(|x| x.id.as_str() == id);
        Ok(found.map(|x| x.password))
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<UserDto>> {
        let user1 = create_test_admin_user()?;
        let user2 = create_test_user()?;
        let users = vec![user1, user2];
        let found = users.into_iter().find(|x| x.username.as_str() == username);
        Ok(found.map(|x| x.into()))
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
