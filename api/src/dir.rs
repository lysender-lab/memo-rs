use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{DbSnafu, MaxDirsReachedSnafu, ValidationSnafu};
use crate::state::AppState;
use db2::dir::{MAX_DIRS, NewDir, UpdateDir};
use memo::dir::DirDto;
use memo::validators::flatten_errors;

pub async fn create_dir(state: &AppState, bucket_id: &str, data: &NewDir) -> Result<DirDto> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // Limit the number of directories per bucket
    let count = state.db.dirs.count(bucket_id).await.context(DbSnafu)?;
    ensure!(count < MAX_DIRS as i64, MaxDirsReachedSnafu,);

    // Directory name must be unique for the bucket
    let existing = state
        .db
        .dirs
        .find_by_name(bucket_id, data.name.as_str())
        .await
        .context(DbSnafu)?;

    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: "Directory name already exists".to_string(),
        }
    );

    state.db.dirs.create(bucket_id, data).await.context(DbSnafu)
}

pub async fn update_dir(state: &AppState, id: &str, data: &UpdateDir) -> Result<bool> {
    let errors = data.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    state.db.dirs.update(id, data).await.context(DbSnafu)
}

pub async fn delete_dir(state: &AppState, id: &str) -> Result<()> {
    // Do not delete if there are still files inside
    let file_count = state.db.files.count_by_dir(id).await.context(DbSnafu)?;
    ensure!(
        file_count == 0,
        ValidationSnafu {
            msg: "Cannot delete directory with files inside".to_string(),
        }
    );

    state.db.dirs.delete(id).await.context(DbSnafu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dir() {
        let data = NewDir {
            name: "hello-world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_ok());

        let data = NewDir {
            name: "hello_world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());

        let data = NewDir {
            name: "".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());
    }
}
