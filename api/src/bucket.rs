use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, DbSnafu, MaxBucketsReachedSnafu, StorageSnafu,
    ValidationSnafu,
};
use crate::schema::buckets::{self, dsl};
use crate::state::AppState;
use db::bucket::MAX_BUCKETS_PER_CLIENT;
use memo::{bucket::BucketDto, utils::generate_id, validators::flatten_errors};

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

pub async fn delete_bucket(state: &AppState, id: &str) -> Result<()> {
    // Do not delete if there are still directories inside
    let dir_count = state.db.dirs.count(id).await.context(DbSnafu)?;
    ensure!(
        dir_count == 0,
        ValidationSnafu {
            msg: "Cannot delete bucket with directories inside".to_string(),
        }
    );

    state.db.buckets.delete(id).await.context(DbSnafu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bucket() {
        let data = NewBucket {
            name: "hello-world".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_ok());

        let data = NewBucket {
            name: "hello_world".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());

        let data = NewBucket {
            name: "".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());
    }
}
