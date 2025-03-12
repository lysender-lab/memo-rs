use validator::Validate;

use super::{
    Actor, ActorPayload, AuthResponse, Credentials, create_auth_token, verify_auth_token,
    verify_password,
};

use crate::{
    Error, Result,
    clients::get_client,
    users::{find_user_by_username, get_user},
    validators::flatten_errors,
    web::server::AppState,
};

pub async fn authenticate(state: &AppState, credentials: &Credentials) -> Result<AuthResponse> {
    if let Err(errors) = credentials.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    let db_pool = state.db_pool.clone();

    // Validate user
    let user = find_user_by_username(&db_pool, &credentials.username).await?;
    let Some(user) = user else {
        return Err(Error::InvalidPassword);
    };

    if &user.status != "active" {
        return Err(Error::InactiveUser);
    }

    // Validate client
    let client = get_client(&db_pool, &user.client_id).await?;
    let Some(client) = client else {
        return Err(Error::InvalidClient);
    };

    if &client.status != "active" {
        return Err(Error::InvalidClient);
    }

    // Validate password
    let _ = verify_password(&credentials.password, &user.password)?;

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
    let db_pool = state.db_pool.clone();
    let client = get_client(&db_pool, &actor.client_id).await?;
    let Some(client) = client else {
        return Err(Error::InvalidClient);
    };
    if &client.status != "active" {
        return Err(Error::InvalidClient);
    }
    let user = get_user(&db_pool, &actor.id).await?;
    let Some(user) = user else {
        return Err(Error::UserNotFound);
    };
    if &user.client_id != &client.id {
        return Err(Error::UserNotFound);
    }

    Ok(Actor::new(actor, user.into()))
}
