use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use memo::dto::client::ClientDto;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::auth::user::count_client_users;
use crate::bucket::{count_client_buckets, find_client_bucket, get_bucket};
use crate::schema::clients::{self, dsl};
use memo::{Error, Result, utils::generate_id, validators::flatten_errors};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::clients)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Client {
    pub id: String,
    pub name: String,
    pub default_bucket_id: Option<String>,
    pub status: String,
    pub admin: Option<i32>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewClient {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::anyname"))]
    pub name: String,

    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::uuid"))]
    pub default_bucket_id: Option<String>,

    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::status"))]
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Validate, AsChangeset)]
#[diesel(table_name = crate::schema::clients)]
pub struct UpdateClient {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::anyname"))]
    pub name: Option<String>,

    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::uuid"))]
    pub default_bucket_id: Option<Option<String>>,

    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::status"))]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, AsChangeset)]
#[diesel(table_name = crate::schema::clients)]
pub struct UpdateClientBucket {
    #[diesel(treat_none_as_null = true)]
    pub default_bucket_id: Option<String>,
}

impl From<ClientDto> for Client {
    fn from(dto: ClientDto) -> Self {
        Client {
            id: dto.id,
            name: dto.name,
            default_bucket_id: dto.default_bucket_id,
            status: dto.status,
            admin: if dto.admin { Some(1) } else { Some(0) },
            created_at: dto.created_at,
        }
    }
}

impl From<Client> for ClientDto {
    fn from(client: Client) -> Self {
        ClientDto {
            id: client.id,
            name: client.name,
            default_bucket_id: client.default_bucket_id,
            status: client.status,
            admin: match client.admin {
                Some(1) => true,
                _ => false,
            },
            created_at: client.created_at,
        }
    }
}

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
            Err(e) => Err(format!("Error reading clients: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn find_admin_client(db_pool: &Pool) -> Result<Option<Client>> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let conn_result = db
        .interact(move |conn| {
            dsl::clients
                .filter(dsl::admin.eq(Some(1)))
                .select(Client::as_select())
                .first::<Client>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(item) => Ok(item),
            Err(e) => Err(format!("Error reading clients: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn create_client(db_pool: &Pool, data: &NewClient, admin: bool) -> Result<Client> {
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
        return Err(Error::ValidationError(
            "Client name already exists".to_string(),
        ));
    }

    if let Some(bucket_id) = data.default_bucket_id.clone() {
        let bucket = find_client_bucket(db_pool, bucket_id.as_str(), &data.name).await?;
        if bucket.is_none() {
            return Err(Error::ValidationError(
                "Default bucket not found".to_string(),
            ));
        }
    }

    let today = chrono::Utc::now().timestamp();
    let admin = if admin { Some(1) } else { Some(0) };
    let client = Client {
        id: generate_id(),
        name: data.name.clone(),
        default_bucket_id: data.default_bucket_id.clone(),
        status: data.status.clone(),
        admin,
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
            Err(e) => Err(format!("Error creating client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn get_client(db_pool: &Pool, id: &str) -> Result<Option<ClientDto>> {
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
            Ok(item) => Ok(item.map(|item| item.into())),
            Err(e) => Err(format!("Error reading clients: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn update_client(db_pool: &Pool, id: &str, data: &UpdateClient) -> Result<bool> {
    if let Err(errors) = data.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Client name must be unique
    if let Some(name) = data.name.clone() {
        if let Some(existing) = find_client_by_name(db_pool, &name).await? {
            if existing.id != id {
                return Err(Error::ValidationError(
                    "Client name already exists".to_string(),
                ));
            }
        }
    }

    // We can't tell whether we are setting default bucket to null or skipping it
    // Will just use a separate function for that
    if let Some(bucket_id) = data.default_bucket_id.clone() {
        if let Some(bid) = bucket_id {
            let bucket = get_bucket(db_pool, &bid).await?;
            if bucket.is_none() {
                return Err(Error::ValidationError(
                    "Default bucket not found".to_string(),
                ));
            }
        }
    }

    let id = id.to_string();
    let data_copy = data.clone();
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::clients)
                .filter(dsl::id.eq(id.as_str()))
                .set(data_copy)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => Err(format!("Error updating client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn find_client_by_name(pool: &Pool, name: &str) -> Result<Option<ClientDto>> {
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
            Ok(item) => Ok(item.map(|item| item.into())),
            Err(e) => Err(format!("Error finding client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
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
            Err(e) => Err(format!("Error counting clients: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
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
            Err(e) => Err(format!("Error updating client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}

pub async fn delete_client(db_pool: &Pool, id: &str) -> Result<()> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let Some(client) = get_client(db_pool, id).await? else {
        return Err(Error::ValidationError("Client not found".to_string()));
    };
    if client.admin {
        return Err(Error::ValidationError(
            "Cannot delete admin client".to_string(),
        ));
    }

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
            Err(e) => Err(format!("Error deleting client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
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
            Err(e) => Err(format!("Error updating client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
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
            Err(e) => Err(format!("Error updating client: {}", e).into()),
        },
        Err(e) => Err(format!("Error using the db connection: {}", e).into()),
    }
}
