use async_trait::async_trait;
use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use memo::client::ClientDto;
use serde::Deserialize;
use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, MaxClientsReachedSnafu, ValidationSnafu,
};
use crate::schema::clients::{self, dsl};
use crate::state::AppState;
use memo::{utils::generate_id, validators::flatten_errors};

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
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

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ClientDefaultBucket {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::uuid"))]
    pub default_bucket_id: Option<String>,
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

pub async fn create_client(state: &AppState, data: &NewClient, admin: bool) -> Result<Client> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // Limit the number of clients because we are poor!
    let count = state.db.clients.count().await?;
    ensure!(count < MAX_CLIENTS as i64, MaxClientsReachedSnafu,);

    state.db.clients.create(data, admin).await
}

pub async fn update_client(state: &AppState, id: &str, data: &UpdateClient) -> Result<bool> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // We can't tell whether we are setting default bucket to null or skipping it
    // Will just use a separate function for that
    if let Some(bucket_id) = data.default_bucket_id.clone() {
        if let Some(bid) = bucket_id {
            let bucket = state.db.buckets.get(&bid).await?;
            ensure!(
                bucket.is_some(),
                ValidationSnafu {
                    msg: "Default bucket not found".to_string(),
                }
            );
        }
    }

    state.db.clients.update(id, data).await
}

pub async fn delete_client(state: &AppState, id: &str) -> Result<()> {
    let Some(client) = state.db.clients.get(id).await? else {
        return ValidationSnafu {
            msg: "Client not found".to_string(),
        }
        .fail();
    };

    ensure!(
        !client.admin,
        ValidationSnafu {
            msg: "Cannot delete admin client".to_string(),
        }
    );

    let bucket_count = state.db.buckets.count_by_client(id).await?;
    ensure!(
        bucket_count == 0,
        ValidationSnafu {
            msg: "Client still has buckets".to_string(),
        }
    );

    let users_count = state.db.users.count_by_client(id).await?;
    ensure!(
        users_count == 0,
        ValidationSnafu {
            msg: "Client still has users".to_string(),
        }
    );

    state.db.clients.delete(id).await
}

#[async_trait]
pub trait ClientRepoable: Send + Sync {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<Client>>;

    async fn find_admin(&self) -> Result<Option<Client>>;

    async fn create(&self, data: &NewClient, admin: bool) -> Result<Client>;

    async fn get(&self, id: &str) -> Result<Option<ClientDto>>;

    async fn update(&self, id: &str, data: &UpdateClient) -> Result<bool>;

    async fn find_by_name(&self, name: &str) -> Result<Option<ClientDto>>;

    async fn count(&self) -> Result<i64>;

    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct ClientRepo {
    db_pool: Pool,
}

impl ClientRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl ClientRepoable for ClientRepo {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<Client>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let client_id_copy = client_id.clone();
        let select_res = db
            .interact(move |conn| {
                let mut query = dsl::clients.into_boxed();
                if let Some(cid) = client_id_copy {
                    query = query.filter(dsl::id.eq(cid));
                }

                query
                    .select(Client::as_select())
                    .order(dsl::name.asc())
                    .load::<Client>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(items)
    }

    async fn find_admin(&self) -> Result<Option<Client>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let select_res = db
            .interact(move |conn| {
                dsl::clients
                    .filter(dsl::admin.eq(Some(1)))
                    .select(Client::as_select())
                    .first::<Client>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(item)
    }

    async fn create(&self, data: &NewClient, admin: bool) -> Result<Client> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // Client name must be unique
        let existing = self.find_by_name(&data.name).await?;
        ensure!(
            existing.is_none(),
            ValidationSnafu {
                msg: "Client name already exists".to_string(),
            }
        );

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
        let insert_res = db
            .interact(move |conn| {
                diesel::insert_into(clients::table)
                    .values(&client_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = insert_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(client)
    }

    async fn get(&self, id: &str) -> Result<Option<ClientDto>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let cid = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::clients
                    .find(cid)
                    .select(Client::as_select())
                    .first::<Client>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(item.map(|item| item.into()))
    }

    async fn update(&self, id: &str, data: &UpdateClient) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // Client name must be unique
        if let Some(name) = data.name.clone() {
            if let Some(existing) = self.find_by_name(&name).await? {
                ensure!(
                    &existing.id == id,
                    ValidationSnafu {
                        msg: "Client name already exists".to_string(),
                    }
                );
            }
        }

        let id = id.to_string();
        let data_copy = data.clone();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::clients)
                    .filter(dsl::id.eq(id.as_str()))
                    .set(data_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let item = update_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(item > 0)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<ClientDto>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let name_copy = name.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::clients
                    .filter(dsl::name.eq(name_copy.as_str()))
                    .select(Client::as_select())
                    .first::<Client>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(item.map(|item| item.into()))
    }

    async fn count(&self) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let count_res = db
            .interact(move |conn| dsl::clients.select(count_star()).get_result::<i64>(conn))
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(count)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let id = id.to_string();
        let delete_res = db
            .interact(move |conn| {
                diesel::delete(dsl::clients.filter(dsl::id.eq(id.as_str()))).execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "clients".to_string(),
        })?;

        Ok(())
    }
}

#[cfg(test)]
pub const TEST_CLIENT_ID: &'static str = "0196d19e01b1745980a8419edd88e3d1";

#[cfg(test)]
pub const TEST_ADMIN_CLIENT_ID: &'static str = "0196d1a2784a72959c97eef5dbc69dc7";

#[cfg(test)]
pub const TEST_NEW_CLIENT_ID: &'static str = "0196d1a2784a72959c97eef5dbc69dc7";

#[cfg(test)]
pub struct ClientTestRepo {}

#[cfg(test)]
pub fn create_test_client() -> Client {
    let today = chrono::Utc::now().timestamp();
    Client {
        id: TEST_CLIENT_ID.to_string(),
        name: "Test Client".to_string(),
        default_bucket_id: None,
        status: "active".to_string(),
        admin: None,
        created_at: today,
    }
}

#[cfg(test)]
pub fn create_test_admin_client() -> Client {
    let today = chrono::Utc::now().timestamp();
    Client {
        id: TEST_ADMIN_CLIENT_ID.to_string(),
        name: "Test Admin Client".to_string(),
        default_bucket_id: None,
        status: "active".to_string(),
        admin: Some(1),
        created_at: today,
    }
}

#[cfg(test)]
pub fn create_test_new_client() -> Client {
    let today = chrono::Utc::now().timestamp();
    Client {
        id: TEST_NEW_CLIENT_ID.to_string(),
        name: "Test New Client".to_string(),
        default_bucket_id: None,
        status: "active".to_string(),
        admin: None,
        created_at: today,
    }
}

#[cfg(test)]
#[async_trait]
impl ClientRepoable for ClientTestRepo {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<Client>> {
        let client1 = create_test_client();
        let client2 = create_test_admin_client();
        let clients = vec![client1, client2];
        match client_id {
            Some(cid) => {
                let filtered: Vec<Client> = clients
                    .into_iter()
                    .filter(|x| x.id.as_str() == cid)
                    .collect();
                Ok(filtered)
            }
            None => Ok(clients),
        }
    }

    async fn find_admin(&self) -> Result<Option<Client>> {
        Ok(Some(create_test_admin_client()))
    }

    async fn create(&self, _data: &NewClient, _admin: bool) -> Result<Client> {
        Ok(create_test_new_client())
    }

    async fn get(&self, id: &str) -> Result<Option<ClientDto>> {
        let clients = self.list(None).await?;
        let found = clients.into_iter().find(|x| x.id.as_str() == id);
        Ok(found.map(|x| x.into()))
    }

    async fn update(&self, _id: &str, _data: &UpdateClient) -> Result<bool> {
        Ok(true)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<ClientDto>> {
        let clients = self.list(None).await?;
        let found = clients.into_iter().find(|x| x.name.as_str() == name);
        Ok(found.map(|x| x.into()))
    }

    async fn count(&self) -> Result<i64> {
        Ok(2)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
