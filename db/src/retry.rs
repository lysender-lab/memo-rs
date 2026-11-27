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
            ) if attempt < max_attempts && is_write_contention(source) => {
                sleep(delay).await;
                delay = (delay * 2).min(max_delay);
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn is_write_contention(source: &turso::Error) -> bool {
    match source {
        turso::Error::Busy(_) | turso::Error::BusySnapshot(_) => true,
        // Turso 0.8.4 exposes MVCC write-write conflicts as a generic error.
        turso::Error::Error(message) => message == "Write-write conflict",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::future::ready;

    use super::*;

    #[tokio::test]
    async fn retries_write_write_conflict() {
        let mut attempts = 0;
        let result = retry_contention(2, || {
            attempts += 1;
            ready(if attempts == 1 {
                Err(Error::DbTransaction {
                    source: turso::Error::Error("Write-write conflict".into()),
                })
            } else {
                Ok(())
            })
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 2);
    }

    #[tokio::test]
    async fn does_not_retry_unrelated_generic_error() {
        let mut attempts = 0;
        let result = retry_contention(2, || {
            attempts += 1;
            ready(Err::<(), _>(Error::DbTransaction {
                source: turso::Error::Error("cannot commit - no transaction is active".into()),
            }))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }
}
