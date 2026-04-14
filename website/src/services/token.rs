use base64::prelude::*;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use snafu::ensure;

use crate::error::Base64DecodeSnafu;
use crate::error::JwtClaimsParseSnafu;
use crate::{Error, Result, error::CsrfTokenSnafu};

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub fn create_csrf_token(subject: &str, secret: &str) -> Result<String> {
    // Limit up to 1 hour only
    let exp = Utc::now() + Duration::hours(1);

    let claims = Claims {
        sub: subject.to_string(),
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

pub fn verify_csrf_token(token: &str, secret: &str) -> Result<String> {
    let Ok(decoded) = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) else {
        return Err(Error::CsrfToken);
    };

    ensure!(!decoded.claims.sub.is_empty(), CsrfTokenSnafu);
    Ok(decoded.claims.sub)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthClaims {
    pub sub: String,
    pub oid: String,
    pub roles: String,
    pub scope: String,
    pub exp: usize,
}

pub fn decode_auth_token(token: &str) -> Result<AuthClaims> {
    let chunks: Vec<&str> = token.split('.').collect();
    if let Some(data_chunk) = chunks.get(1) {
        let decoded = BASE64_URL_SAFE_NO_PAD
            .decode(*data_chunk)
            .context(Base64DecodeSnafu)?;

        let claims: AuthClaims = serde_json::from_slice(&decoded).context(JwtClaimsParseSnafu)?;
        return Ok(claims);
    }

    Err(Error::InvalidAuthToken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_token() {
        // Generate token
        let token = create_csrf_token("example", "secret").expect("Token should be generated");
        assert!(token.len() > 0);
        println!("Token: {}", token);

        // Validate it back
        let value = verify_csrf_token(&token, "secret").expect("Token should be verified");
        assert_eq!(value, "example".to_string());
    }

    #[test]
    fn test_expired_token() {
        let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJleGFtcGxlIiwiZXhwIjoxNzIxMDk1MDIyfQ.7ddeJN3Tys_8kc8a02umkNLv42lPHIoSDaqmi-WjRhE".to_string();
        let result = verify_csrf_token(&token, "secret");
        assert!(result.is_err());
    }
}
