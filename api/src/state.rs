use axum::extract::FromRef;
use std::sync::Arc;

use crate::{
    Result,
    config::Config,
    db::{DbMapper, create_db_mapper},
    storage::{CloudStorable, StorageClient},
};

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Config,
    pub storage_client: Arc<dyn CloudStorable>,
    pub db: Arc<DbMapper>,
}

pub async fn create_app_state(config: &Config) -> Result<AppState> {
    let storage_client = StorageClient::new(config.cloud.credentials.as_str()).await?;
    let db = create_db_mapper(config.db.url.as_str());
    Ok(AppState {
        config: config.clone(),
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
    })
}
