use base64::prelude::*;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use memo::file::{ALLOWED_IMAGE_TYPES, UploadDto};
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileUploadClaims {
    pub sub: String,
    pub upload_id: String,
    pub orig_filename: String,
    pub new_filename: String,
    pub content_type: String,
    pub size: i64,
    pub exp: usize,
}

impl FileUploadClaims {
    pub fn is_image(&self) -> bool {
        ALLOWED_IMAGE_TYPES.contains(&self.content_type.as_str())
    }
}

pub fn create_upload_token(upload: &UploadDto, secret: &str) -> Result<String> {
    // File creation still reads metadata directly from the token.
    let claims = FileUploadClaims {
        sub: "upload".to_string(),
        upload_id: upload.id.clone(),
        orig_filename: upload.original_filename.clone(),
        new_filename: upload.filename.clone(),
        content_type: upload.content_type.clone(),
        size: upload.size,
        exp: upload.expires_at as usize,
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

    ensure!(
        !decoded.claims.sub.is_empty() && !decoded.claims.upload_id.trim().is_empty(),
        UploadTokenSnafu
    );
    Ok(decoded.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn upload_token_requires_nonempty_upload_id() {
        let secret = "test-secret";
        let mut claims = json!({
            "sub": "upload",
            "orig_filename": "photo.jpg",
            "new_filename": "stored-photo.jpg",
            "content_type": "image/jpeg",
            "size": 10,
            "exp": (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
        });

        for upload_id in [None, Some(""), Some("   "), Some("upl_test")] {
            if let Some(upload_id) = upload_id {
                claims["upload_id"] = json!(upload_id);
            }
            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(secret.as_bytes()),
            )
            .unwrap();
            let result = verify_upload_token(&token, secret);
            if upload_id == Some("upl_test") {
                assert_eq!(result.unwrap().upload_id, "upl_test");
            } else {
                assert!(matches!(result, Err(Error::UploadToken)));
            }
        }
    }
}
