use serde::{Deserialize, Serialize};
use snafu::ResultExt;

use crate::{
    Result,
    error::{HttpClientSnafu, HttpResponseParseSnafu},
    run::AppState,
};

const VERIFY_URL: &str =
    "https://recaptchaenterprise.googleapis.com/v1/projects/lysender-misc-project/assessments?key=";

#[derive(Serialize)]
struct CaptchaEvent {
    token: String,

    #[serde(rename = "expectedAction")]
    expected_token: String,

    #[serde(rename = "siteKey")]
    site_key: String,
}

#[derive(Serialize)]
struct CaptchaPayload {
    event: CaptchaEvent,
}

#[derive(Deserialize)]
struct CaptchaResponse {
    #[allow(dead_code)]
    name: String,

    #[allow(dead_code)]
    event: CaptchaResponseEvent,

    #[serde(rename = "riskAnalysis")]
    #[allow(dead_code)]
    risk_analysis: RiskAnalysis,

    #[serde(rename = "tokenProperties")]
    #[allow(dead_code)]
    token_properties: TokenProperties,
}

#[derive(Deserialize)]
struct CaptchaResponseEvent {
    #[allow(dead_code)]
    token: String,
}

#[derive(Deserialize)]
struct RiskAnalysis {
    #[allow(dead_code)]
    score: f64,

    #[allow(dead_code)]
    reasons: Vec<String>,
}

#[derive(Deserialize)]
struct TokenProperties {
    #[allow(dead_code)]
    valid: bool,

    #[serde(rename = "invalidReason")]
    #[allow(dead_code)]
    invalid_reason: String,
}

#[derive(Deserialize)]
struct CaptchaError {
    code: u16,
    message: String,
    status: String,
}

#[derive(Deserialize)]
struct CaptchaErrorResponse {
    error: CaptchaError,
}

pub async fn validate_catpcha(state: &AppState, response: &str) -> Result<()> {
    let site_key = state
        .config
        .captcha_site_key
        .as_deref()
        .ok_or_else(|| "Captcha site key is not configured".to_string())?;
    let api_key = state
        .config
        .captcha_api_key
        .as_deref()
        .ok_or_else(|| "Captcha API key is not configured".to_string())?;

    let post_body = CaptchaPayload {
        event: CaptchaEvent {
            token: response.to_string(),
            expected_token: "login".to_string(),
            site_key: site_key.to_string(),
        },
    };

    let url = format!("{}{}", VERIFY_URL, api_key);
    let response = state
        .client
        .post(url)
        .json(&post_body)
        .send()
        .await
        .context(HttpClientSnafu {
            msg: "Unable to validate captcha".to_string(),
        })?;

    if !response.status().is_success() {
        // Try to parse error response as JSON
        let err_json =
            response
                .json::<CaptchaErrorResponse>()
                .await
                .context(HttpResponseParseSnafu {
                    msg: "Unable to parse captcha error response",
                })?;

        return Err(format!("Unable to validate captcha: {}", err_json.error.message).into());
    }

    Ok(())
}
