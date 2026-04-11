use std::sync::Arc;

use db2::create_db_mapper;
use snafu::ResultExt;
use tracing::info;

use crate::Result;
use crate::config::Config;
use crate::error::DbSnafu;

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
    Ok(())
}

async fn migrate_users(state: &State) -> Result<()> {
    Ok(())
}

async fn migrate_buckets(state: &State) -> Result<()> {
    Ok(())
}

async fn migrate_dirs(state: &State) -> Result<()> {
    Ok(())
}

async fn migrate_files(state: &State) -> Result<()> {
    Ok(())
}
