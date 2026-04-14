use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{DbSnafu, MaxBucketsReachedSnafu, StorageSnafu, ValidationSnafu};
use crate::state::AppState;
use db::bucket::{MAX_BUCKETS_PER_CLIENT, NewBucket, UpdateBucket};
use memo::{bucket::BucketDto, validators::flatten_errors};

pub async fn create_bucket(
    state: &AppState,
    client_id: &str,
    data: &NewBucket,
) -> Result<BucketDto> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // Limit the number of buckets per client
    let count = state
        .db
        .buckets
        .count_by_client(client_id)
        .await
        .context(DbSnafu)?;

    ensure!(
        count < MAX_BUCKETS_PER_CLIENT as i64,
        MaxBucketsReachedSnafu
    );

    // Bucket name must be unique for the client
    let existing = state
        .db
        .buckets
        .find_by_name(client_id, &data.name)
        .await
        .context(DbSnafu)?;

    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: "Bucket name already exists".to_string(),
        }
    );

    // Validate against the cloud storage
    let _ = state
        .storage_client
        .read_bucket(&data.name)
        .await
        .context(StorageSnafu)?;

    state
        .db
        .buckets
        .create(client_id, data)
        .await
        .context(DbSnafu)
}

pub async fn update_bucket(state: &AppState, id: &str, data: &UpdateBucket) -> Result<bool> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    state.db.buckets.update(id, data).await.context(DbSnafu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bucket() {
        let data = NewBucket {
            name: "hello-world".to_string(),
            label: "Hello World".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_ok());

        let data = NewBucket {
            name: "hello_world".to_string(),
            label: "Hello World".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());

        let data = NewBucket {
            name: "".to_string(),
            label: "".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());
    }
}
