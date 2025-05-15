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

#[cfg(test)]
pub fn create_test_app_state() -> AppState {
    use std::path::PathBuf;

    use crate::config::{CloudConfig, DbConfig, ServerConfig};
    use crate::db::create_test_db_mapper;
    use crate::storage::StorageTestClient;

    let config = Config {
        jwt_secret: "0196d1dbbfd87819b9183f14ac3ed485".to_string(),
        upload_dir: PathBuf::new(),
        cloud: CloudConfig {
            project_id: "test-cloud-project-id".to_string(),
            credentials: "test-credentials-file.json".to_string(),
        },
        server: ServerConfig { port: 43700 },
        db: DbConfig {
            url: "-url".to_string(),
        },
    };

    let storage_client = StorageTestClient::new();
    let db = create_test_db_mapper();

    AppState {
        config,
        storage_client: Arc::new(storage_client),
        db: Arc::new(db),
    }
}
