use axum::{http::Response, response::IntoResponse};
use tower_cookies::{Cookie, Cookies};

use super::AUTH_TOKEN_COOKIE;

pub async fn logout_handler(cookies: Cookies) -> impl IntoResponse {
    cookies.remove(Cookie::new(AUTH_TOKEN_COOKIE, ""));

    Response::builder()
        .status(200)
        .header("HX-Redirect", "/login")
        .body("Log in".to_string())
        .expect("Response builder must succeed")
}
