use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct LoginFormPayload {
    #[validate(length(min = 4, max = 50))]
    pub username: String,

    #[validate(length(min = 8, max = 100))]
    pub password: String,

    #[serde(rename = "g-recaptcha-response")]
    pub g_recaptcha_response: Option<String>,
}
