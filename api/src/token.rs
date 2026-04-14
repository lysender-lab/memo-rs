use base64::prelude::*;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

use crate::{
    Result,
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
