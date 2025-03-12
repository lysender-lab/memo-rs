use deadpool_diesel::sqlite::Pool;

use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use tracing::error;
use validator::Validate;

use crate::buckets::{count_client_buckets, get_bucket};
use crate::schema::clients::{self, dsl};
use crate::users::count_client_users;
use crate::util::generate_id;
use crate::validators::flatten_errors;
use crate::{Error, Result};

use super::{Client, NewClient, UpdateClientBucket};

// Can't have too many clients
const MAX_CLIENTS: i32 = 10;

pub async fn list_clients(db_pool: &Pool) -> Result<Vec<Client>> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let conn_result = db
        .interact(move |conn| {
            dsl::clients
                .select(Client::as_select())
                .order(dsl::name.asc())
                .load::<Client>(conn)
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(items) => Ok(items),
            Err(e) => {
                error!("{e}");
                Err("Error reading clients".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

pub async fn create_client(db_pool: &Pool, data: &NewClient) -> Result<Client> {
    if let Err(errors) = data.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Limit the number of clients because we are poor!
    let _ = match count_clients(db_pool).await {
        Ok(count) => {
            if count >= MAX_CLIENTS as i64 {
                return Err(Error::ValidationError(
                    "Maximum number of clients reached".to_string(),
                ));
            }
        }
        Err(e) => return Err(e),
    };

    // Client name must be unique
    if let Some(_) = find_client_by_name(db_pool, &data.name).await? {
        return Err(Error::ValidationError("Client already exists".to_string()));
    }

    let data_copy = data.clone();
    let today = chrono::Utc::now().timestamp();
    let client = Client {
        id: generate_id(),
        name: data_copy.name,
        default_bucket_id: None,
        status: "active".to_string(),
        created_at: today,
    };

    let client_copy = client.clone();
    let conn_result = db
        .interact(move |conn| {
            diesel::insert_into(clients::table)
                .values(&client_copy)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(insert_res) => match insert_res {
            Ok(_) => Ok(client),
            Err(e) => {
                error!("{}", e);
                Err("Error creating client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn get_client(db_pool: &Pool, id: &str) -> Result<Option<Client>> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let cid = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::clients
                .find(cid)
                .select(Client::as_select())
                .first::<Client>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(item) => Ok(item),
            Err(e) => {
                error!("{e}");
                Err("Error reading clients".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

pub async fn find_client_by_name(pool: &Pool, name: &str) -> Result<Option<Client>> {
    let Ok(db) = pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let name_copy = name.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::clients
                .filter(dsl::name.eq(name_copy.as_str()))
                .select(Client::as_select())
                .first::<Client>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(item) => Ok(item),
            Err(e) => {
                error!("{}", e);
                Err("Error finding client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn count_clients(db_pool: &Pool) -> Result<i64> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let conn_result = db
        .interact(move |conn| dsl::clients.select(count_star()).get_result::<i64>(conn))
        .await;

    match conn_result {
        Ok(count_res) => match count_res {
            Ok(count) => Ok(count),
            Err(e) => {
                error!("{}", e);
                Err("Error counting clients".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn update_client_status(db_pool: &Pool, id: &str, status: &str) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let id = id.to_string();
    let status = status.to_string();
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::clients)
                .filter(dsl::id.eq(id.as_str()))
                .set(dsl::status.eq(status))
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn delete_client(db_pool: &Pool, id: &str) -> Result<()> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let bucket_count = count_client_buckets(db_pool, id).await?;
    if bucket_count > 0 {
        return Err(Error::ValidationError(
            "Client still has buckets".to_string(),
        ));
    }

    let users_count = count_client_users(db_pool, id).await?;
    if users_count > 0 {
        return Err(Error::ValidationError("Client still has users".to_string()));
    }

    let id = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            diesel::delete(dsl::clients.filter(dsl::id.eq(id.as_str()))).execute(conn)
        })
        .await;

    match conn_result {
        Ok(delete_res) => match delete_res {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{}", e);
                Err("Error deleting client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn set_client_default_bucket(db_pool: &Pool, id: &str, bucket_id: &str) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Ensure that bucket exists and is owned by the client
    let bucket = get_bucket(db_pool, bucket_id).await?;
    let Some(bucket) = bucket else {
        return Err(Error::ValidationError("Bucket not found".to_string()));
    };

    if bucket.client_id.as_str() != id {
        return Err(Error::ValidationError(
            "Bucket not owned by client".to_string(),
        ));
    }

    let id = id.to_string();
    let bucket_id = bucket_id.to_string();
    let data = UpdateClientBucket {
        default_bucket_id: Some(bucket_id),
    };

    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::clients)
                .filter(dsl::id.eq(id.as_str()))
                .set(data)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn unset_client_default_bucket(db_pool: &Pool, id: &str) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let id = id.to_string();
    let data = UpdateClientBucket {
        default_bucket_id: None,
    };
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::clients)
                .filter(dsl::id.eq(id.as_str()))
                .set(data)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating client".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}
