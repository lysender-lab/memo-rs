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
        .move_to_dir(&new_dir_id, old_dir_id)
        .await
        .context(DbSnafu)?;

    // Delete old dir
    state.db.dirs.delete(old_dir_id).await.context(DbSnafu)?;

    Ok(())
}
