use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use super::models::ActorPayload;
use crate::{Error, Result};

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    cid: String,
    bid: Option<String>,
    scope: String,
    exp: usize,
}

// Duration in seconds
const EXP_DURATION: i64 = 60 * 60 * 24 * 7; // 1 week

pub fn create_auth_token(actor: &ActorPayload, secret: &str) -> Result<String> {
    let exp = Utc::now() + Duration::seconds(EXP_DURATION);
    let data = actor.clone();

    let claims = Claims {
        sub: data.id,
        cid: data.client_id,
        bid: data.default_bucket_id,
        scope: data.scope,
        exp: exp.timestamp() as usize,
    };

    let Ok(token) = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) else {
        return Err("Error creating JWT token".into());
    };

    Ok(token)
}

pub fn verify_auth_token(token: &str, secret: &str) -> Result<ActorPayload> {
    let Ok(decoded) = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) else {
        return Err(Error::InvalidAuthToken);
    };

    if decoded.claims.sub.len() == 0 {
        return Err(Error::InvalidAuthToken);
    }
    if decoded.claims.scope.len() == 0 {
        return Err(Error::InvalidAuthToken);
    }

    Ok(ActorPayload {
        id: decoded.claims.sub,
        client_id: decoded.claims.cid,
        default_bucket_id: decoded.claims.bid,
        scope: decoded.claims.scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_token() {
        // Generate token
        let actor = ActorPayload {
            id: "thor01".to_string(),
            client_id: "client01".to_string(),
            default_bucket_id: None,
            scope: "auth files".to_string(),
        };
        let token = create_auth_token(&actor, "secret").unwrap();
        assert!(token.len() > 0);

        // Validate it back
        let actor = verify_auth_token(&token, "secret").unwrap();
        assert_eq!(actor.id, "thor01".to_string());
        assert_eq!(actor.client_id, "client01".to_string());
        assert_eq!(actor.scope, "auth files".to_string());
    }

    #[test]
    fn test_expired_token() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWJqZWN0IjoidGhvcjAxIiwic2NvcGUiOiJhdXRoIGZpbGVzIiwiZXhwIjoxNzE5MDc2MTI2fQ.ep8nXWWHS75MxoOY_yB4m0uoWgxCz1bPNvTPIgourcQ".to_string();
        let result = verify_auth_token(&token, "secret");
        assert!(result.is_err());
    }
}
