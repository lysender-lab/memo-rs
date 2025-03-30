use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use validator::Validate;

use super::password::hash_password;
use crate::Result2;
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

pub async fn list_users(db_pool: &Pool, client_id: &str) -> Result2<Vec<User>> {
    let db = db_pool.get().await.context(DbPoolSnafu)?;

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

pub async fn create_user(db_pool: &Pool, client_id: &str, data: &NewUser) -> Result2<User> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    let db = db_pool.get().await.context(DbPoolSnafu)?;
    let count = count_client_users(db_pool, client_id).await?;
    ensure!(count < MAX_USERS_PER_CLIENT as i64, MaxUsersReachedSnafu);

    // Username must be unique
    let existing = find_user_by_username(db_pool, &data.username).await?;
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

pub async fn get_user(pool: &Pool, id: &str) -> Result2<Option<User>> {
    let db = pool.get().await.context(DbPoolSnafu)?;

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

pub async fn find_user_by_username(pool: &Pool, username: &str) -> Result2<Option<User>> {
    let db = pool.get().await.context(DbPoolSnafu)?;

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

pub async fn count_client_users(db_pool: &Pool, client_id: &str) -> Result2<i64> {
    let db = db_pool.get().await.context(DbPoolSnafu)?;

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

pub async fn update_user_status(db_pool: &Pool, id: &str, status: &str) -> Result2<bool> {
    let db = db_pool.get().await.context(DbPoolSnafu)?;

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

pub async fn update_user_password(db_pool: &Pool, id: &str, password: &str) -> Result2<bool> {
    let db = db_pool.get().await.context(DbPoolSnafu)?;

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

pub async fn delete_user(db_pool: &Pool, id: &str) -> Result2<()> {
    let db = db_pool.get().await.context(DbPoolSnafu)?;

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
