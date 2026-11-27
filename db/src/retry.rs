use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;

use crate::{Error, Result};

pub(crate) const WRITE_ATTEMPTS: usize = 5;

pub(crate) async fn retry_contention<T, Attempt>(
    max_attempts: usize,
    mut operation: impl FnMut() -> Attempt,
) -> Result<T>
where
    Attempt: Future<Output = Result<T>>,
{
    let mut attempt = 1;
    let mut delay = Duration::from_millis(100);
    let max_delay = Duration::from_secs(2);
    loop {
        match operation().await {
            Err(
                Error::DbPrepare { ref source }
                | Error::DbStatement { ref source }
                | Error::DbResult { ref source }
                | Error::DbTransaction { ref source },
            ) if attempt < max_attempts
                && matches!(
                    source,
                    turso::Error::Busy(_) | turso::Error::BusySnapshot(_)
                ) =>
            {
                sleep(delay).await;
                delay = (delay * 2).min(max_delay);
                attempt += 1;
            }
            result => return result,
        }
    }
}
