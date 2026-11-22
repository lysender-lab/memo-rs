use std::time::Duration;

use snafu::ResultExt;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use crate::{Result, error::CronSnafu, state::AppState};

pub async fn start(state: AppState) -> Result<JobScheduler> {
    let scheduler = JobScheduler::new().await.context(CronSnafu)?;
    let job = Job::new_repeated_async(Duration::from_secs(10), move |_id, _scheduler| {
        Box::pin(log_cron_handler(state.clone()))
    })
    .context(CronSnafu)?;

    scheduler.add(job).await.context(CronSnafu)?;
    scheduler.start().await.context(CronSnafu)?;
    Ok(scheduler)
}

async fn log_cron_handler(_state: AppState) {
    info!("hello from cron");
}

pub async fn shutdown(mut scheduler: JobScheduler) -> Result<()> {
    scheduler.shutdown().await.context(CronSnafu)
}
