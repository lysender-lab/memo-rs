use serde::Serialize;

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
