use std::sync::Arc;

use db2::create_db_mapper;
use db2::turso_decode::{FromTursoRow, row_integer, row_text};
use db2::turso_params::{integer_param, new_query_params, opt_text_param, text_param};
use memo::bucket::BucketDto;
use memo::client::ClientDto;
use memo::dir::DirDto;
use snafu::ResultExt;
use tracing::info;
use turso::Row;

use crate::Result;
use crate::config::Config;
use crate::error::DbSnafu;

struct LegacyUserRow {
    id: String,
    client_id: String,
    username: String,
    password: String,
    status: String,
    roles: String,
    created_at: i64,
    updated_at: i64,
}

impl FromTursoRow for LegacyUserRow {
    fn from_row(row: &Row) -> db2::Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
            client_id: row_text(row, 1)?,
            username: row_text(row, 2)?,
            password: row_text(row, 3)?,
            status: row_text(row, 4)?,
            roles: row_text(row, 5)?,
            created_at: row_integer(row, 6)?,
            updated_at: row_integer(row, 7)?,
        })
    }
}

pub struct State {
    source_db: Arc<db2::DbMapper>,
    target_db: Arc<db2::DbMapper>,
}

pub async fn run() -> Result<()> {
    let config = Config::build()?;

    // Open original database (sqlite but compatible with turso)
    let source_db = create_db_mapper(config.source_db_path.as_path())
        .await
        .context(DbSnafu)?;

    // Open target database (turso)
    let target_db = create_db_mapper(config.target_db_path.as_path())
        .await
        .context(DbSnafu)?;

    let state = State {
        source_db: Arc::new(source_db),
        target_db: Arc::new(target_db),
    };

    info!("Starting database migration...");
    info!("Source DB: {:?}", config.source_db_path);
    info!("Target DB: {:?}", config.target_db_path);

    // Create schema first before running the migration
    // For each tables from the original database, copy each row to the new database
    // from sqlite (diesel) to tursodb

    check_target_tables(&state).await?;

    migrate_clients(&state).await?;
    migrate_users(&state).await?;
    migrate_buckets(&state).await?;
    migrate_dirs(&state).await?;
    migrate_files(&state).await?;

    Ok(())
}

async fn check_target_tables(state: &State) -> Result<()> {
    // Ensure no existing clients
    let clients_count_query = "SELECT COUNT(*) AS total_count FROM clients".to_string();
    let clients_count = state
        .target_db
        .any
        .count_query(clients_count_query, Vec::new())
        .await
        .context(DbSnafu)?;

    assert!(clients_count == 0, "Target database should have no clients");

    // Ensure no existing users
    let users_count_query = "SELECT COUNT(*) AS total_count FROM users".to_string();
    let users_count = state
        .target_db
        .any
        .count_query(users_count_query, Vec::new())
        .await
        .context(DbSnafu)?;

    assert!(users_count == 0, "Target database should have no users");

    // Ensure no existing buckets
    let buckets_count_query = "SELECT COUNT(*) AS total_count FROM buckets".to_string();
    let buckets_count = state
        .target_db
        .any
        .count_query(buckets_count_query, Vec::new())
        .await
        .context(DbSnafu)?;

    assert!(buckets_count == 0, "Target database should have no buckets");

    // Ensure no existing dirs
    let dirs_count_query = "SELECT COUNT(*) AS total_count FROM dirs".to_string();
    let dirs_count = state
        .target_db
        .any
        .count_query(dirs_count_query, Vec::new())
        .await
        .context(DbSnafu)?;

    assert!(dirs_count == 0, "Target database should have no dirs");

    // Ensure no existing files
    let files_count_query = "SELECT COUNT(*) AS total_count FROM files".to_string();
    let files_count = state
        .target_db
        .any
        .count_query(files_count_query, Vec::new())
        .await
        .context(DbSnafu)?;

    assert!(files_count == 0, "Target database should have no files");

    Ok(())
}

async fn migrate_clients(state: &State) -> Result<()> {
    info!("Migrating clients...");
    // There are only a couple of clients, so just fetch them all
    let clients_query = r#"
        SELECT
            id,
            name,
            default_bucket_id,
            status,
            admin,
            created_at
        FROM
            clients
    "#
    .to_string();

    let clients: Vec<ClientDto> = state
        .source_db
        .any
        .query(clients_query, Vec::new())
        .await
        .context(DbSnafu)?;

    let clients_count = clients.len();

    let insert_query = r#"
        INSERT INTO clients (
            id,
            name,
            default_bucket_id,
            status,
            admin,
            created_at
        ) VALUES (
            :id,
            :name,
            :default_bucket_id,
            :status,
            :admin,
            :created_at
        )
    "#
    .to_string();

    for client in clients.into_iter() {
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", client.id));
        q_params.push(text_param(":name", client.name));
        q_params.push(opt_text_param(
            ":default_bucket_id",
            client.default_bucket_id,
        ));
        q_params.push(text_param(":status", client.status));
        q_params.push(integer_param(
            ":admin",
            match client.admin {
                true => 1,
                false => 0,
            },
        ));
        q_params.push(integer_param(":created_at", client.created_at));

        state
            .target_db
            .any
            .execute(insert_query.clone(), q_params)
            .await
            .context(DbSnafu)?;
    }

    info!("Migrated {} clients...", clients_count);

    Ok(())
}

async fn migrate_users(state: &State) -> Result<()> {
    info!("Migrating users...");

    let users_query = r#"
        SELECT
            id,
            client_id,
            username,
            password,
            status,
            roles,
            created_at,
            updated_at
        FROM
            users
    "#
    .to_string();

    let users: Vec<LegacyUserRow> = state
        .source_db
        .any
        .query(users_query, Vec::new())
        .await
        .context(DbSnafu)?;

    let users_count = users.len();

    let insert_query = r#"
        INSERT INTO users (
            id,
            client_id,
            username,
            password,
            status,
            roles,
            created_at,
            updated_at
        ) VALUES (
            :id,
            :client_id,
            :username,
            :password,
            :status,
            :roles,
            :created_at,
            :updated_at
        )
    "#
    .to_string();

    for user in users.into_iter() {
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", user.id));
        q_params.push(text_param(":client_id", user.client_id));
        q_params.push(text_param(":username", user.username));
        q_params.push(text_param(":password", user.password));
        q_params.push(text_param(":status", user.status));
        q_params.push(text_param(":roles", user.roles));
        q_params.push(integer_param(":created_at", user.created_at));
        q_params.push(integer_param(":updated_at", user.updated_at));

        state
            .target_db
            .any
            .execute(insert_query.clone(), q_params)
            .await
            .context(DbSnafu)?;
    }

    info!("Migrated {} users...", users_count);

    Ok(())
}

async fn migrate_buckets(state: &State) -> Result<()> {
    info!("Migrating buckets...");

    let buckets_query = r#"
        SELECT
            id,
            client_id,
            name,
            label,
            images_only,
            created_at
        FROM
            buckets
    "#
    .to_string();

    let buckets: Vec<BucketDto> = state
        .source_db
        .any
        .query(buckets_query, Vec::new())
        .await
        .context(DbSnafu)?;

    let buckets_count = buckets.len();

    let insert_query = r#"
        INSERT INTO buckets (
            id,
            client_id,
            name,
            label,
            images_only,
            created_at
        ) VALUES (
            :id,
            :client_id,
            :name,
            :label,
            :images_only,
            :created_at
        )
    "#
    .to_string();

    for bucket in buckets.into_iter() {
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", bucket.id));
        q_params.push(text_param(":client_id", bucket.client_id));
        q_params.push(text_param(":name", bucket.name));
        q_params.push(text_param(":label", bucket.label));
        q_params.push(integer_param(
            ":images_only",
            match bucket.images_only {
                true => 1,
                false => 0,
            },
        ));
        q_params.push(integer_param(":created_at", bucket.created_at));

        state
            .target_db
            .any
            .execute(insert_query.clone(), q_params)
            .await
            .context(DbSnafu)?;
    }

    info!("Migrated {} buckets...", buckets_count);

    Ok(())
}

async fn migrate_dirs(state: &State) -> Result<()> {
    info!("Migrating dirs...");

    let mut migrated_count: usize = 0;
    let limit: i64 = 100;
    let mut offset: i64 = 0;

    let insert_query = r#"
        INSERT INTO dirs (
            id,
            bucket_id,
            name,
            label,
            file_count,
            created_at,
            updated_at
        ) VALUES (
            :id,
            :bucket_id,
            :name,
            :label,
            :file_count,
            :created_at,
            :updated_at
        )
    "#
    .to_string();

    loop {
        let query = r#"
            SELECT
                id,
                bucket_id,
                name,
                label,
                file_count,
                created_at,
                updated_at
            FROM
                dirs
            ORDER BY
                created_at ASC,
                id ASC
            LIMIT :limit OFFSET :offset
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(integer_param(":limit", limit));
        q_params.push(integer_param(":offset", offset));

        let dirs: Vec<DirDto> = state
            .source_db
            .any
            .query(query, q_params)
            .await
            .context(DbSnafu)?;

        let page_len = dirs.len();
        if page_len == 0 {
            break;
        }

        for dir in dirs.into_iter() {
            let mut insert_params = new_query_params();
            insert_params.push(text_param(":id", dir.id));
            insert_params.push(text_param(":bucket_id", dir.bucket_id));
            insert_params.push(text_param(":name", dir.name));
            insert_params.push(text_param(":label", dir.label));
            insert_params.push(integer_param(":file_count", dir.file_count as i64));
            insert_params.push(integer_param(":created_at", dir.created_at));
            insert_params.push(integer_param(":updated_at", dir.updated_at));

            state
                .target_db
                .any
                .execute(insert_query.clone(), insert_params)
                .await
                .context(DbSnafu)?;
        }

        migrated_count += page_len;

        if page_len < limit as usize {
            break;
        }

        offset += limit;
    }

    info!("Migrated {} dirs...", migrated_count);

    Ok(())
}

async fn migrate_files(state: &State) -> Result<()> {
    Ok(())
}
