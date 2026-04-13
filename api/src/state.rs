use axum::extract::FromRef;
use moka::sync::Cache;
use snafu::ResultExt;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    Result,
    config::Config,
    error::{DbSnafu, StorageSnafu},
};
use storage::StorageClient;
use yaas::actor::Actor;

use db::{DbMapper, create_db_mapper};

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Config,
    pub storage_client: Arc<StorageClient>,
    pub db: Arc<DbMapper>,
    pub auth_cache: Cache<String, Actor>,
}

pub async fn create_app_state(config: &Config) -> Result<AppState> {
    let storage_client = StorageClient::new(config.cloud.credentials.as_str())
        .await
        .context(StorageSnafu)?;

    let db_file = config.db.dir.join("default").join("memo.db");
    let db = create_db_mapper(db_file.as_path()).await.context(DbSnafu)?;

    let auth_cache = Cache::builder()
        .time_to_live(Duration::from_secs(10 * 60))
        .time_to_idle(Duration::from_secs(60))
        .max_capacity(100)
        .build();

    Ok(AppState {
        config: config.clone(),
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
        auth_cache,
    })
}
