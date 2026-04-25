use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{DbSnafu, ValidationSnafu};
use crate::state::AppState;
use db::bucket::UpdateBucket;
use memo::validators::flatten_errors;

pub async fn update_bucket(state: &AppState, id: &str, data: &UpdateBucket) -> Result<bool> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    state.bucket_cache.remove(id);
    state.db.buckets.update(id, data).await.context(DbSnafu)
}
