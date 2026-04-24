use base64::prelude::*;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use snafu::ensure;

use crate::error::UploadTokenSnafu;
use crate::{
    Error, Result,
    error::{Base64DecodeSnafu, JwtClaimsParseSnafu},
};

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

    Err("Invalid auth token.".into())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileUploadClaims {
    pub sub: String,
    pub orig_filename: String,
    pub new_filename: String,
    pub content_type: String,
    pub exp: usize,
}

pub fn create_upload_token(
    orig_filename: String,
    new_filename: String,
    content_type: String,
    secret: &str,
) -> Result<String> {
    // Limit up to 1 hour only
    let exp = Utc::now() + Duration::hours(1);

    let claims = FileUploadClaims {
        sub: "upload".to_string(),
        orig_filename: orig_filename,
        new_filename: new_filename,
        content_type: content_type,
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

pub fn verify_upload_token(token: &str, secret: &str) -> Result<FileUploadClaims> {
    let Ok(decoded) = decode::<FileUploadClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) else {
        return Err(Error::UploadToken);
    };

    ensure!(!decoded.claims.sub.is_empty(), UploadTokenSnafu);
    Ok(decoded.claims)
}
