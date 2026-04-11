use axum::extract::FromRef;
use snafu::ResultExt;
use std::sync::Arc;

use crate::{
    Result,
    config::Config,
    error::{DbSnafu, StorageSnafu},
};
use storage::{CloudStorable, StorageClient};

use db2::{DbMapper, create_db_mapper};

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Config,
    pub storage_client: Arc<dyn CloudStorable>,
    pub db: Arc<DbMapper>,
}

pub async fn create_app_state(config: &Config) -> Result<AppState> {
    let storage_client = StorageClient::new(config.cloud.credentials.as_str())
        .await
        .context(StorageSnafu)?;

    let db_file = config.db.dir.join("default").join("yaas.db");
    let db = create_db_mapper(db_file.as_path()).await.context(DbSnafu)?;

    Ok(AppState {
        config: config.clone(),
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
    })
}
