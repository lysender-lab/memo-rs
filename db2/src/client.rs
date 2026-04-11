use memo::client::ClientDto;
use serde::Deserialize;
use snafu::ResultExt;
use turso::{Connection, Row};
use validator::Validate;

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, opt_row_text, row_integer, row_text,
};
use crate::turso_params::{
    integer_param, new_query_params, opt_integer_param, opt_text_param, text_param,
};
use memo::utils::generate_id;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Deserialize, Validate)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateClientBucket {
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

impl FromTursoRow for ClientDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
            name: row_text(row, 1)?,
            default_bucket_id: opt_row_text(row, 4)?,
            status: row_text(row, 2)?,
            admin: match row_integer(row, 3)? {
                1 => true,
                _ => false,
            },
            created_at: row_integer(row, 6)?,
        })
    }
}

// Can't have too many clients
pub const MAX_CLIENTS: i32 = 10;

pub struct ClientRepo {
    db_pool: Connection,
}

impl ClientRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }
}

impl ClientRepo {
    pub async fn list(&self, client_id: Option<String>) -> Result<Vec<ClientDto>> {
        let mut query = r#"
            SELECT
                id,
                name,
                default_bucket_id,
                status,
                admin,
                created_at
            FROM clients
        "#
        .to_string();

        let mut q_params = new_query_params();

        if let Some(cid) = client_id {
            query.push_str(" WHERE id = :id");
            q_params.push(text_param("id", cid));
        }

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<ClientDto> = collect_rows(&mut rows).await?;

        Ok(items)
    }

    async fn find_admin(&self) -> Result<Option<ClientDto>> {
        let query = r#"
            SELECT
                id,
                name,
                default_bucket_id,
                status,
                admin,
                created_at
            FROM clients
            WHERE admin = 1
            LIMIT 1
        "#
        .to_string();

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row({}).await;
        let dto: Option<ClientDto> = collect_row(row_result)?;
        Ok(dto)
    }

    async fn create(&self, data: NewClient, admin: bool) -> Result<ClientDto> {
        let today = chrono::Utc::now().timestamp();
        let admin: Option<i64> = if admin { Some(1) } else { Some(0) };

        let query = r#"
            INSERT INTO clients
            (
                id,
                name,
                default_bucket_id,
                status,
                admin,
                created_at,
            )
            VALUES
            (
                :id,
                :name,
                :default_bucket_id,
                :status,
                :admin,
                :created_at,
            )
        "#;

        let id = generate_id();
        let mut params = new_query_params();
        params.push(text_param(":id", id.clone()));
        params.push(text_param(":name", data.name.clone()));
        params.push(opt_text_param(
            ":default_bucket_id",
            data.default_bucket_id.clone(),
        ));
        params.push(text_param(":status", data.status.clone()));
        params.push(opt_integer_param(":admin", admin.clone()));
        params.push(integer_param(":created_at", today));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(params).await.context(DbStatementSnafu)?;

        Ok(ClientDto {
            id,
            name: data.name,
            default_bucket_id: data.default_bucket_id,
            status: data.status,
            admin: matches!(admin, Some(1)),
            created_at: today,
        })
    }

    async fn get(&self, id: &str) -> Result<Option<ClientDto>> {
        let query = r#"
            SELECT
                id,
                name,
                default_bucket_id,
                status,
                admin,
                created_at
            FROM clients
            WHERE id = :id
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<ClientDto> = collect_row(row_result)?;
        Ok(dto)
    }

    async fn update(&self, id: &str, data: UpdateClient) -> Result<bool> {
        if data.name.is_none() && data.default_bucket_id.is_none() && data.status.is_none() {
            return Ok(false);
        }

        let mut query = "UPDATE clients SET ".to_string();
        let mut set_parts: Vec<&str> = Vec::new();
        let mut q_params = new_query_params();

        if let Some(name) = data.name {
            set_parts.push("name = :name");
            q_params.push(text_param(":name", name));
        }

        if let Some(default_bucket_id) = data.default_bucket_id {
            set_parts.push("default_bucket_id = :default_bucket_id");
            q_params.push(opt_text_param(":default_bucket_id", default_bucket_id));
        }

        if let Some(status) = data.status {
            set_parts.push("status = :status");
            q_params.push(text_param(":status", status));
        }

        query.push_str(&set_parts.join(", "));
        query.push_str(" WHERE id = :id");
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(affected > 0)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<ClientDto>> {
        let query = r#"
            SELECT
                id,
                name,
                default_bucket_id,
                status,
                admin,
                created_at
            FROM clients
            WHERE name = :name
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":name", name.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<ClientDto> = collect_row(row_result)?;
        Ok(dto)
    }

    async fn count(&self) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) AS total_count
            FROM clients
        "#
        .to_string();

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row({}).await;
        collect_count(row_result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let query = "DELETE clients WHERE id = :id".to_string();
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }
}
