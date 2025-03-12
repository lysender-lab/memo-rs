use deadpool_diesel::sqlite::Pool;

use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use tracing::error;
use validator::Validate;

use crate::auth::hash_password;
use crate::roles::to_roles;
use crate::schema::users::{self, dsl};
use crate::util::generate_id;
use crate::validators::flatten_errors;
use crate::{Error, Result};

use super::{NewUser, User};

const MAX_USERS_PER_CLIENT: i32 = 50;

pub async fn list_users(db_pool: &Pool, client_id: &str) -> Result<Vec<User>> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let client_id = client_id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::users
                .filter(dsl::client_id.eq(&client_id))
                .select(User::as_select())
                .order(dsl::username.asc())
                .load::<User>(conn)
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(items) => Ok(items),
            Err(e) => {
                error!("{e}");
                Err("Error reading users".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

pub async fn create_user(db_pool: &Pool, client_id: &str, data: &NewUser) -> Result<User> {
    if let Err(errors) = data.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Limit the number of directories per bucket
    let _ = match count_client_users(db_pool, client_id).await {
        Ok(count) => {
            if count >= MAX_USERS_PER_CLIENT as i64 {
                return Err(Error::ValidationError(
                    "Maximum number of users reached".to_string(),
                ));
            }
        }
        Err(e) => return Err(e),
    };

    // Username must be unique
    if let Some(_) = find_user_by_username(db_pool, &data.username).await? {
        return Err(Error::ValidationError(
            "Username already exists".to_string(),
        ));
    }

    // Roles must be all valid
    let roles: Vec<String> = data.roles.split(",").map(|item| item.to_string()).collect();
    // Validate roles
    let _ = to_roles(roles)?;

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
    let conn_result = db
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values(&user_copy)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(insert_res) => match insert_res {
            Ok(_) => Ok(dir),
            Err(e) => {
                error!("{}", e);
                Err("Error creating a user".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn get_user(pool: &Pool, id: &str) -> Result<Option<User>> {
    let Ok(db) = pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let id = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::users
                .find(&id)
                .select(User::as_select())
                .first::<User>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(user) => Ok(user),
            Err(e) => {
                error!("{e}");
                Err("Error reading users".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

pub async fn find_user_by_username(pool: &Pool, username: &str) -> Result<Option<User>> {
    let Ok(db) = pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let username = username.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::users
                .filter(dsl::username.eq(&username))
                .select(User::as_select())
                .first::<User>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(user) => Ok(user),
            Err(e) => {
                error!("{}", e);
                Err("Error finding user".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn count_client_users(db_pool: &Pool, client_id: &str) -> Result<i64> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let client_id = client_id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::users
                .filter(dsl::client_id.eq(&client_id))
                .select(count_star())
                .get_result::<i64>(conn)
        })
        .await;

    match conn_result {
        Ok(count_res) => match count_res {
            Ok(count) => Ok(count),
            Err(e) => {
                error!("{}", e);
                Err("Error counting users".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn update_user_status(db_pool: &Pool, id: &str, status: &str) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let id = id.to_string();
    let status = status.to_string();
    let today = chrono::Utc::now().timestamp();
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::users)
                .filter(dsl::id.eq(&id))
                .set((dsl::status.eq(&status), dsl::updated_at.eq(today)))
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(affected) => Ok(affected > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating user".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn update_user_password(db_pool: &Pool, id: &str, password: &str) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let id = id.to_string();
    let today = chrono::Utc::now().timestamp();
    let hashed = hash_password(&password)?;
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::users)
                .filter(dsl::id.eq(&id))
                .set((dsl::password.eq(&hashed), dsl::updated_at.eq(today)))
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(affected) => Ok(affected > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating user password".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn delete_user(db_pool: &Pool, id: &str) -> Result<()> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // It is okay to delete user even if there are potential references
    // to created buckets, dirs or files
    let id = id.to_string();
    let conn_result = db
        .interact(move |conn| diesel::delete(dsl::users.filter(dsl::id.eq(&id))).execute(conn))
        .await;

    match conn_result {
        Ok(delete_res) => match delete_res {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{}", e);
                Err("Error deleting user".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}
