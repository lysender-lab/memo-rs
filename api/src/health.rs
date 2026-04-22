use serde::Serialize;
use std::sync::Arc;
use tracing::error;

use crate::{Result, config::Config};
use db::DbMapper;

#[derive(Serialize)]
pub struct LiveStatus {
    pub status: String,
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub message: String,
    pub checks: HealthChecks,
}

#[derive(Serialize)]
pub struct HealthChecks {
    pub cloud_storage: String,
    pub database: String,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.checks.is_healthy()
    }
}

impl HealthChecks {
    pub fn new() -> Self {
        Self {
            cloud_storage: "DOWN".to_string(),
            database: "DOWN".to_string(),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.cloud_storage == "UP" && self.database == "UP"
    }
}

pub async fn check_liveness() -> Result<LiveStatus> {
    // Nothing much to check, if it hits this function, it's alive
    Ok(LiveStatus {
        status: "UP".to_string(),
    })
}

pub async fn check_readiness(config: &Config, db: Arc<DbMapper>) -> Result<HealthStatus> {
    let checks = perform_checks(config, db).await?;
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

async fn perform_checks(_config: &Config, db: Arc<DbMapper>) -> Result<HealthChecks> {
    let mut checks = HealthChecks::new();

    checks.database = check_database(db).await?;

    Ok(checks)
}

async fn check_database(db: Arc<DbMapper>) -> Result<String> {
    match db.buckets.test_read().await {
        Ok(_) => Ok("UP".to_string()),
        Err(e) => {
            let msg = format!("{}", e);
            error!(msg);
            Ok("DOWN".to_string())
        }
    }
}
