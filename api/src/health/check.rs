use deadpool_diesel::sqlite::Pool;
use tracing::error;

use crate::{
    Result,
    buckets::test_read_bucket,
    config::Config,
    storage::{create_storage_client, test_list_hmac_keys},
};

use super::{HealthChecks, HealthStatus, LiveStatus};

pub async fn check_liveness() -> Result<LiveStatus> {
    // Nothing much to check, if it hits this function, it's alive
    Ok(LiveStatus {
        status: "UP".to_string(),
    })
}

pub async fn check_readiness(config: &Config, db_pool: &Pool) -> Result<HealthStatus> {
    let checks = perform_checks(config, db_pool).await?;
    let mut status = "DOWN".to_string();
    let mut message = "One or more health checks are failing".to_string();

    if checks.is_healthy() {
        status = "UP".to_string();
        message = "All health checks are passing".to_string();
    }

    Ok(HealthStatus {
        status,
        message,
        checks,
    })
}

async fn perform_checks(config: &Config, db_pool: &Pool) -> Result<HealthChecks> {
    let mut checks = HealthChecks::new();

    checks.cloud_storage = check_cloud_storage(config).await?;
    checks.database = check_database(db_pool).await?;

    Ok(checks)
}

async fn check_cloud_storage(config: &Config) -> Result<String> {
    let client = create_storage_client(config.cloud.credentials.as_str()).await?;
    match test_list_hmac_keys(&client, config.cloud.project_id.as_str()).await {
        Ok(_) => Ok("UP".to_string()),
        Err(e) => {
            let msg = format!("{}", e);
            error!(msg);
            Ok("DOWN".to_string())
        }
    }
}

async fn check_database(db_pool: &Pool) -> Result<String> {
    match test_read_bucket(db_pool).await {
        Ok(_) => Ok("UP".to_string()),
        Err(e) => {
            let msg = format!("{}", e);
            error!(msg);
            Ok("DOWN".to_string())
        }
    }
}
