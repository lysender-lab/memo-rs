use validator::Validate;

use memo::actor::{Actor, ActorPayload, AuthResponse, Credentials};
use password::verify_password;
use snafu::{OptionExt, ResultExt, ensure};
use token::{create_auth_token, verify_auth_token};

use crate::error::{
    DbSnafu, InactiveUserSnafu, InvalidClientSnafu, InvalidPasswordSnafu, UserNotFoundSnafu,
    ValidationSnafu, WhateverSnafu,
};
use crate::{Result, state::AppState};
use memo::validators::flatten_errors;

pub mod token;
pub mod user;

pub async fn authenticate(state: &AppState, credentials: &Credentials) -> Result<AuthResponse> {
    let errors = credentials.validate();
    ensure!(
        errors.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&errors.unwrap_err()),
        }
    );

    // Validate user
    let user = state
        .db
        .users
        .find_by_username(&credentials.username)
        .await
        .context(DbSnafu)?;

    let user = user.context(InvalidPasswordSnafu)?;

    ensure!(&user.status == "active", InactiveUserSnafu);

    // Validate client
    let client = state
        .db
        .clients
        .get(&user.client_id)
        .await
        .context(DbSnafu)?;

    let client = client.context(InvalidClientSnafu)?;
    ensure!(&client.status == "active", InvalidClientSnafu);

    // Validate password
    let password = state
        .db
        .users
        .get_password(&user.id)
        .await
        .context(DbSnafu)?;

    let password = password.context(WhateverSnafu {
        msg: "Unable to re-query user.".to_string(),
    })?;

    ensure!(
        verify_password(&credentials.password, &password).is_ok(),
        InvalidPasswordSnafu
    );

    // Generate a token
    let actor = ActorPayload {
        id: user.id.clone(),
        client_id: client.id.clone(),
        default_bucket_id: client.default_bucket_id.clone(),
        scope: "auth files".to_string(),
    };
    let token = create_auth_token(&actor, &state.config.jwt_secret)?;
    Ok(AuthResponse {
        user: user.into(),
        token,
    })
}

pub async fn authenticate_token(state: &AppState, token: &str) -> Result<Actor> {
    let actor = verify_auth_token(token, &state.config.jwt_secret)?;

    // Validate client
    let client = state
        .db
        .clients
        .get(&actor.client_id)
        .await
        .context(DbSnafu)?;

    let client = client.context(InvalidClientSnafu)?;
    ensure!(&client.status == "active", InvalidClientSnafu);

    let user = state.db.users.get(&actor.id).await.context(DbSnafu)?;
    let user = user.context(UserNotFoundSnafu)?;
    ensure!(&user.client_id == &client.id, UserNotFoundSnafu);

    Ok(Actor::new(actor, user.into()))
}
