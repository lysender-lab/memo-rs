use async_trait::async_trait;
use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use memo::client::ClientDto;
use serde::Deserialize;
use snafu::ResultExt;
use validator::Validate;

use crate::Result;
use crate::error::{DbInteractSnafu, DbPoolSnafu, DbQuerySnafu};
use crate::schema::clients::{self, dsl};
use memo::utils::generate_id;

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
            admin: matches!(client.admin, Some(1)),
            created_at: client.created_at,
        }
    }
}

// Can't have too many clients
pub const MAX_CLIENTS: i32 = 10;

#[async_trait]
pub trait ClientStore: Send + Sync {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<ClientDto>>;

    async fn find_admin(&self) -> Result<Option<ClientDto>>;

    async fn create(&self, data: &NewClient, admin: bool) -> Result<ClientDto>;

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
impl ClientStore for ClientRepo {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<ClientDto>> {
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

        let dtos: Vec<ClientDto> = items.into_iter().map(|x| x.into()).collect();

        Ok(dtos)
    }

    async fn find_admin(&self) -> Result<Option<ClientDto>> {
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

        Ok(item.map(|x| x.into()))
    }

    async fn create(&self, data: &NewClient, admin: bool) -> Result<ClientDto> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

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

        Ok(client.into())
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

#[cfg(feature = "test")]
pub const TEST_CLIENT_ID: &'static str = "0196d19e01b1745980a8419edd88e3d1";

#[cfg(feature = "test")]
pub const TEST_ADMIN_CLIENT_ID: &'static str = "0196d1a2784a72959c97eef5dbc69dc7";

#[cfg(feature = "test")]
pub const TEST_NEW_CLIENT_ID: &'static str = "0196d1a2784a72959c97eef5dbc69dc7";

#[cfg(feature = "test")]
pub struct ClientTestRepo {}

#[cfg(feature = "test")]
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

#[cfg(feature = "test")]
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

#[cfg(feature = "test")]
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

#[cfg(feature = "test")]
#[async_trait]
impl ClientStore for ClientTestRepo {
    async fn list(&self, client_id: Option<String>) -> Result<Vec<ClientDto>> {
        let client1 = create_test_client();
        let client2 = create_test_admin_client();
        let clients = vec![client1, client2];
        match client_id {
            Some(cid) => {
                let filtered: Vec<ClientDto> = clients
                    .into_iter()
                    .filter(|x| x.id.as_str() == cid)
                    .map(|x| x.into())
                    .collect();
                Ok(filtered)
            }
            None => {
                let dtos: Vec<ClientDto> = clients.into_iter().map(|x| x.into()).collect();
                Ok(dtos)
            }
        }
    }

    async fn find_admin(&self) -> Result<Option<ClientDto>> {
        Ok(Some(create_test_admin_client().into()))
    }

    async fn create(&self, _data: &NewClient, _admin: bool) -> Result<ClientDto> {
        Ok(create_test_new_client().into())
    }

    async fn get(&self, id: &str) -> Result<Option<ClientDto>> {
        let clients = self.list(None).await?;
        let found = clients.into_iter().find(|x| x.id.as_str() == id);
        Ok(found)
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
