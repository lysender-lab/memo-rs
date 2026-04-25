use memo::utils::{IdPrefix, generate_prefixed_id};
use snafu::ResultExt;
use std::sync::Arc;
use tracing::info;

use crate::config::Config;
use crate::{Result, error::DbSnafu};
use db::{DbMapper, create_db_mapper};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DbMapper>,
}

pub async fn run() -> Result<()> {
    let config = Config::build()?;

    let db_file = config.db_dir.join("default").join("memo.db");
    let db = create_db_mapper(db_file.as_path()).await.context(DbSnafu)?;

    let state = AppState { db: Arc::new(db) };

    migrate_dirs(&state).await?;
    migrate_files(&state).await?;

    Ok(())
}

async fn migrate_dirs(state: &AppState) -> Result<()> {
    // List all dirs
    let dirs = state.db.dirs.list_dir_ids().await.context(DbSnafu)?;

    info!("Migrating {} dirs", dirs.len());

    for dir_id in dirs {
        migrate_dir(state, &dir_id).await?;
    }

    Ok(())
}

async fn migrate_dir(state: &AppState, old_dir_id: &str) -> Result<()> {
    // Get dir
    let dir = state.db.dirs.get(old_dir_id).await.context(DbSnafu)?;
    let dir = dir.expect("Dir should exist");

    // Create a new dir copy with a new ID
    let new_dir_id = generate_prefixed_id(IdPrefix::Dir);
    let mut new_dir = dir.clone();
    new_dir.id = new_dir_id.clone();

    state.db.dirs.create_full(new_dir).await.context(DbSnafu)?;

    // Update all files in the dir to point to the new dir ID
    state
        .db
        .files
        .move_to_dir(old_dir_id, &new_dir_id)
        .await
        .context(DbSnafu)?;

    // Delete old dir
    state.db.dirs.delete(old_dir_id).await.context(DbSnafu)?;

    Ok(())
}

async fn migrate_files(state: &AppState) -> Result<()> {
    info!("Migrating files...");

    // List all files
    let files = state.db.files.list_file_ids().await.context(DbSnafu)?;

    info!("Migrating {} files", files.len());

    for file_id in files.iter() {
        migrate_file(state, &file_id).await?;
    }

    info!("Finished migrating {} files", files.len());

    Ok(())
}

async fn migrate_file(state: &AppState, old_file_id: &str) -> Result<()> {
    // Create a new file copy with a new ID
    let new_file_id = generate_prefixed_id(IdPrefix::File);

    // Update file id
    state
        .db
        .files
        .update_id(old_file_id, &new_file_id)
        .await
        .context(DbSnafu)?;

    Ok(())
}
