use memo::client::ClientDto;
use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{DbSnafu, MaxClientsReachedSnafu, ValidationSnafu};
use crate::state::AppState;
use db::client::{MAX_CLIENTS, NewClient, UpdateClient};
use memo::validators::flatten_errors;

pub async fn create_client(state: &AppState, data: &NewClient, admin: bool) -> Result<ClientDto> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // Limit the number of clients because we are poor!
    let count = state.db.clients.count().await.context(DbSnafu)?;
    ensure!(count < MAX_CLIENTS as i64, MaxClientsReachedSnafu,);

    // Client name must be unique
    let existing = state
        .db
        .clients
        .find_by_name(&data.name)
        .await
        .context(DbSnafu)?;

    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: "Client name already exists".to_string(),
        }
    );

    state.db.clients.create(data, admin).await.context(DbSnafu)
}

pub async fn update_client(state: &AppState, id: &str, data: &UpdateClient) -> Result<bool> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // We can't tell whether we are setting default bucket to null or skipping it
    // Will just use a separate function for that
    if let Some(bucket_id) = data.default_bucket_id.clone() {
        if let Some(bid) = bucket_id {
            let bucket = state.db.buckets.get(&bid).await.context(DbSnafu)?;
            ensure!(
                bucket.is_some(),
                ValidationSnafu {
                    msg: "Default bucket not found".to_string(),
                }
            );
        }
    }

    // Client name must be unique
    if let Some(name) = data.name.clone() {
        if let Some(existing) = state
            .db
            .clients
            .find_by_name(&name)
            .await
            .context(DbSnafu)?
        {
            ensure!(
                &existing.id == id,
                ValidationSnafu {
                    msg: "Client name already exists".to_string(),
                }
            );
        }
    }

    state.db.clients.update(id, data).await.context(DbSnafu)
}

pub async fn delete_client(state: &AppState, id: &str) -> Result<()> {
    let Some(client) = state.db.clients.get(id).await.context(DbSnafu)? else {
        return ValidationSnafu {
            msg: "Client not found".to_string(),
        }
        .fail();
    };

    ensure!(
        !client.admin,
        ValidationSnafu {
            msg: "Cannot delete admin client".to_string(),
        }
    );

    let bucket_count = state
        .db
        .buckets
        .count_by_client(id)
        .await
        .context(DbSnafu)?;

    ensure!(
        bucket_count == 0,
        ValidationSnafu {
            msg: "Client still has buckets".to_string(),
        }
    );

    let users_count = state.db.users.count_by_client(id).await.context(DbSnafu)?;
    ensure!(
        users_count == 0,
        ValidationSnafu {
            msg: "Client still has users".to_string(),
        }
    );

    state.db.clients.delete(id).await.context(DbSnafu)
}
