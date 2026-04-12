use serde::Deserialize;
use snafu::ResultExt;
use turso::{Connection, Row};
use validator::Validate;

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, HashPasswordSnafu, InvalidRolesSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, row_integer, row_text,
};
use crate::turso_params::{integer_param, new_query_params, text_param};
use memo::role::to_roles;
use memo::user::UserDto;
use memo::utils::generate_id;
use password::hash_password;

#[derive(Debug, Clone)]
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
        let role_list = user.roles.split(',').map(|item| item.to_string()).collect();
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

impl FromTursoRow for UserDto {
    fn from_row(row: &Row) -> Result<Self> {
        let roles_str = row_text(row, 4)?;
        let role_list = roles_str.split(',').map(|item| item.to_string()).collect();
        let roles = to_roles(role_list).context(InvalidRolesSnafu)?;

        Ok(Self {
            id: row_text(row, 0)?,
            client_id: row_text(row, 1)?,
            username: row_text(row, 2)?,
            status: row_text(row, 3)?,
            roles,
            created_at: row_integer(row, 5)?,
            updated_at: row_integer(row, 6)?,
        })
    }
}

struct UserPassword {
    password: String,
}

impl FromTursoRow for UserPassword {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            password: row_text(row, 0)?,
        })
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

pub struct UserRepo {
    db_pool: Connection,
}

impl UserRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    pub async fn list(&self, client_id: &str) -> Result<Vec<UserDto>> {
        let query = r#"
            SELECT
                id,
                client_id,
                username,
                status,
                roles,
                created_at,
                updated_at
            FROM users
            WHERE client_id = :client_id
            ORDER BY username ASC
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":client_id", client_id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<UserDto> = collect_rows(&mut rows).await?;

        Ok(items)
    }

    pub async fn create(&self, client_id: &str, data: &NewUser) -> Result<UserDto> {
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password).context(HashPasswordSnafu)?;

        let id = generate_id();
        let status = "active".to_string();

        let query = r#"
            INSERT INTO users
            (
                id,
                client_id,
                username,
                password,
                status,
                roles,
                created_at,
                updated_at
            )
            VALUES
            (
                :id,
                :client_id,
                :username,
                :password,
                :status,
                :roles,
                :created_at,
                :updated_at
            )
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.clone()));
        q_params.push(text_param(":client_id", client_id.to_owned()));
        q_params.push(text_param(":username", data.username.clone()));
        q_params.push(text_param(":password", hashed));
        q_params.push(text_param(":status", status.clone()));
        q_params.push(text_param(":roles", data.roles.clone()));
        q_params.push(integer_param(":created_at", today));
        q_params.push(integer_param(":updated_at", today));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        let role_list = data.roles.split(',').map(|item| item.to_string()).collect();
        let roles = to_roles(role_list).context(InvalidRolesSnafu)?;

        Ok(UserDto {
            id,
            client_id: client_id.to_owned(),
            username: data.username.clone(),
            status,
            roles,
            created_at: today,
            updated_at: today,
        })
    }

    pub async fn get(&self, id: &str) -> Result<Option<UserDto>> {
        let query = r#"
            SELECT
                id,
                client_id,
                username,
                status,
                roles,
                created_at,
                updated_at
            FROM users
            WHERE id = :id
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<UserDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn get_password(&self, id: &str) -> Result<Option<String>> {
        let query = r#"
            SELECT password
            FROM users
            WHERE id = :id
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<UserPassword> = collect_row(row_result)?;
        Ok(dto.map(|x| x.password))
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserDto>> {
        let query = r#"
            SELECT
                id,
                client_id,
                username,
                status,
                roles,
                created_at,
                updated_at
            FROM users
            WHERE username = :username
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":username", username.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<UserDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) AS total_count
            FROM users
            WHERE client_id = :client_id
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":client_id", client_id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn update_status(&self, id: &str, data: &UpdateUserStatus) -> Result<bool> {
        let today = chrono::Utc::now().timestamp();

        let query = r#"
            UPDATE users
            SET status = :status, updated_at = :updated_at
            WHERE id = :id
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":status", data.status.clone()));
        q_params.push(integer_param(":updated_at", today));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(affected > 0)
    }

    pub async fn update_roles(&self, id: &str, data: &UpdateUserRoles) -> Result<bool> {
        let today = chrono::Utc::now().timestamp();

        let query = r#"
            UPDATE users
            SET roles = :roles, updated_at = :updated_at
            WHERE id = :id
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":roles", data.roles.clone()));
        q_params.push(integer_param(":updated_at", today));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(affected > 0)
    }

    pub async fn update_password(&self, id: &str, data: &UpdateUserPassword) -> Result<bool> {
        let today = chrono::Utc::now().timestamp();
        let hashed = hash_password(&data.password).context(HashPasswordSnafu)?;

        let query = r#"
            UPDATE users
            SET password = :password, updated_at = :updated_at
            WHERE id = :id
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":password", hashed));
        q_params.push(integer_param(":updated_at", today));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(affected > 0)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let query = "DELETE FROM users WHERE id = :id".to_string();
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }
}
