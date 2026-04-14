use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct OauthAuthorizeDto {
    #[validate(length(equal = 36))]
    pub client_id: String,

    #[validate(url)]
    #[validate(length(min = 1, max = 250))]
    pub redirect_uri: String,

    #[validate(length(min = 1, max = 250))]
    pub scope: String,

    #[validate(length(min = 1, max = 250))]
    pub state: String,
}

#[derive(Clone, Serialize)]
pub struct OauthAuthorizationCodeDto {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct OauthTokenRequestDto {
    #[validate(length(equal = 36))]
    pub client_id: String,

    #[validate(length(equal = 36))]
    pub client_secret: String,

    #[validate(length(equal = 36))]
    pub code: String,

    #[validate(length(min = 1, max = 250))]
    pub state: String,

    #[validate(url)]
    #[validate(length(min = 1, max = 250))]
    pub redirect_uri: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OauthTokenResponseDto {
    pub access_token: String,
    pub scope: String,
    pub token_type: String,
}
