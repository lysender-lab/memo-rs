use axum::{Router, body::Body, middleware, response::Response};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, info};

use crate::Result;
use crate::config::Config;
use crate::error::{ErrorInfo, ErrorResponse};
use crate::state::create_app_state;
use crate::web::routes::all_routes;

#[cfg(test)]
use axum_test::TestServer;

pub async fn run_web_server(config: &Config) -> Result<()> {
    let port = config.server.port;
    let state = create_app_state(config).await?;

    let routes_all = Router::new()
        .merge(all_routes(state))
        .layer(middleware::map_response(response_mapper))
        .layer(
            ServiceBuilder::new().layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            ),
        );

    // Setup the server
    let ip = "127.0.0.1";
    let addr = format!("{}:{}", ip, port);
    info!("HTTP server running on {}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, routes_all.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("HTTP server stopped");

    Ok(())
}

async fn response_mapper(res: Response) -> Response {
    let error = res.extensions().get::<ErrorInfo>();
    if let Some(e) = error {
        if e.status_code.is_server_error() {
            // Build the error response
            error!("{}", e.message);
            if let Some(bt) = &e.backtrace {
                error!("{}", bt);
            }
        }

        let body = ErrorResponse {
            status_code: e.status_code.as_u16(),
            message: e.message.as_str(),
            error: e.status_code.canonical_reason().unwrap(),
        };

        return Response::builder()
            .status(e.status_code)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
    }
    res
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
fn create_test_app() -> TestServer {
    use crate::state::create_test_app_state;

    let state = create_test_app_state();

    let app = Router::new()
        .merge(all_routes(state))
        .layer(middleware::map_response(response_mapper));

    TestServer::builder()
        .save_cookies()
        .default_content_type("application/json")
        .expect_success_by_default()
        .mock_transport()
        .build(app)
        .unwrap()
}

#[cfg(test)]
fn create_test_user_auth_token() -> Result<String> {
    use crate::auth::token::create_auth_token;
    use crate::auth::user::TEST_USER_ID;
    use crate::client::TEST_CLIENT_ID;
    use crate::state::create_test_app_state;
    use memo::actor::ActorPayload;

    let state = create_test_app_state();

    let actor = ActorPayload {
        id: TEST_USER_ID.to_string(),
        client_id: TEST_CLIENT_ID.to_string(),
        default_bucket_id: None,
        scope: "auth files".to_string(),
    };

    create_auth_token(&actor, &state.config.jwt_secret)
}

#[cfg(test)]
fn create_test_admin_auth_token() -> Result<String> {
    use crate::auth::token::create_auth_token;
    use crate::auth::user::TEST_ADMIN_USER_ID;
    use crate::client::TEST_ADMIN_CLIENT_ID;
    use crate::state::create_test_app_state;
    use memo::actor::ActorPayload;

    let state = create_test_app_state();

    let actor = ActorPayload {
        id: TEST_ADMIN_USER_ID.to_string(),
        client_id: TEST_ADMIN_CLIENT_ID.to_string(),
        default_bucket_id: None,
        scope: "auth files".to_string(),
    };

    create_auth_token(&actor, &state.config.jwt_secret)
}

#[cfg(test)]
mod tests {
    use crate::{
        auth::user::{TEST_ADMIN_USER_ID, TEST_USER_ID},
        bucket::TEST_BUCKET_ID,
        client::{TEST_ADMIN_CLIENT_ID, TEST_CLIENT_ID},
        dir::{Dir, TEST_DIR_ID},
    };

    use super::*;
    use memo::{
        bucket::BucketDto, client::ClientDto, file::FileDto, pagination::Paginated, user::UserDto,
    };
    use serde_json::json;

    #[tokio::test]
    async fn test_home_page() {
        let server = create_test_app();
        let response = server.get("/").await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_health_live() {
        let server = create_test_app();
        let response = server.get("/health/liveness").await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_login_invalid() {
        let server = create_test_app();
        let response = server
            .post("/auth/token")
            .json(&json!({
                "username": "pythagoras",
                "password": "not-a-strong-password",
            }))
            .expect_failure()
            .await;

        response.assert_status_unauthorized();
    }

    #[tokio::test]
    async fn test_login_admin() {
        let server = create_test_app();
        let response = server
            .post("/auth/token")
            .json(&json!({
                "username": "admin",
                "password": "secret-password",
            }))
            .await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_login_user() {
        let server = create_test_app();
        let response = server
            .post("/auth/token")
            .json(&json!({
                "username": "user",
                "password": "secret-password",
            }))
            .await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_list_clients_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let clients: Vec<ClientDto> = server
            .get("/clients")
            .authorization_bearer(token.as_str())
            .await
            .json();

        // Should only see its own
        assert_eq!(clients.len(), 1);

        let client = clients.first().unwrap();
        assert_eq!(client.id.as_str(), TEST_CLIENT_ID);
    }

    #[tokio::test]
    async fn test_list_clients_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let clients: Vec<ClientDto> = server
            .get("/clients")
            .authorization_bearer(token.as_str())
            .await
            .json();

        // Should see all clients
        assert_eq!(clients.len(), 2);
    }

    #[tokio::test]
    async fn test_user_profile_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let user: UserDto = server
            .get("/user")
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(user.id.as_str(), TEST_USER_ID);
    }

    #[tokio::test]
    async fn test_user_profile_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let user: UserDto = server
            .get("/user")
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(user.id.as_str(), TEST_ADMIN_USER_ID);
    }

    #[tokio::test]
    async fn test_get_user_client_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}", TEST_CLIENT_ID);
        let client: ClientDto = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(client.id.as_str(), TEST_CLIENT_ID);
    }

    #[tokio::test]
    async fn test_get_admin_client_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}", TEST_ADMIN_CLIENT_ID);
        let response = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .expect_failure()
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_get_user_client_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}", TEST_CLIENT_ID);
        let client: ClientDto = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(client.id.as_str(), TEST_CLIENT_ID);
    }

    #[tokio::test]
    async fn test_get_admin_client_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}", TEST_ADMIN_CLIENT_ID);
        let client: ClientDto = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(client.id.as_str(), TEST_ADMIN_CLIENT_ID);
    }

    #[tokio::test]
    async fn test_get_client_not_found_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = "/clients/0196d27b10c47e1abb9aae6cf3eea36a";
        let response = server
            .get(url)
            .authorization_bearer(token.as_str())
            .expect_failure()
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_list_user_buckets_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}/buckets", TEST_CLIENT_ID);
        let buckets: Vec<BucketDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(buckets.len(), 1);
        let bucket = buckets.first().unwrap();
        assert_eq!(bucket.id.as_str(), TEST_BUCKET_ID);
    }

    #[tokio::test]
    async fn test_list_admin_buckets_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}/buckets", TEST_ADMIN_CLIENT_ID);
        let response = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .expect_failure()
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_list_user_buckets_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}/buckets", TEST_CLIENT_ID);
        let buckets: Vec<BucketDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(buckets.len(), 1);
        let bucket = buckets.first().unwrap();
        assert_eq!(bucket.id.as_str(), TEST_BUCKET_ID);
    }

    #[tokio::test]
    async fn test_list_admin_buckets_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}/buckets", TEST_ADMIN_CLIENT_ID);
        let buckets: Vec<BucketDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(buckets.len(), 0);
    }

    #[tokio::test]
    async fn test_get_user_bucket_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}/buckets/{}", TEST_CLIENT_ID, TEST_BUCKET_ID);
        let bucket: BucketDto = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(bucket.id.as_str(), TEST_BUCKET_ID);
    }

    #[tokio::test]
    async fn test_get_user_bucket_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}/buckets/{}", TEST_CLIENT_ID, TEST_BUCKET_ID);
        let bucket: BucketDto = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(bucket.id.as_str(), TEST_BUCKET_ID);
    }

    #[tokio::test]
    async fn test_get_user_bucket_not_found_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/0196d277ffc47800ba5e7ffb6a557f31",
            TEST_CLIENT_ID
        );
        let response = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .expect_failure()
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_list_user_users_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!("/clients/{}/users", TEST_CLIENT_ID);
        let users: Vec<UserDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(users.len(), 1);
        let user = users.first().unwrap();
        assert_eq!(user.id.as_str(), TEST_USER_ID);
    }

    #[tokio::test]
    async fn test_list_user_users_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!("/clients/{}/users", TEST_CLIENT_ID);
        let users: Vec<UserDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(users.len(), 1);
        let user = users.first().unwrap();
        assert_eq!(user.id.as_str(), TEST_USER_ID);
    }

    #[tokio::test]
    async fn test_list_user_dirs_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs",
            TEST_CLIENT_ID, TEST_BUCKET_ID
        );
        let dirs: Paginated<Dir> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dirs.meta.total_records, 1);
        let dir = dirs.data.first().unwrap();
        assert_eq!(dir.id.as_str(), TEST_DIR_ID);
    }

    #[tokio::test]
    async fn test_list_user_dirs_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs",
            TEST_CLIENT_ID, TEST_BUCKET_ID
        );
        let dirs: Paginated<Dir> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dirs.meta.total_records, 1);
        let dir = dirs.data.first().unwrap();
        assert_eq!(dir.id.as_str(), TEST_DIR_ID);
    }

    #[tokio::test]
    async fn test_get_user_dir_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs/{}",
            TEST_CLIENT_ID, TEST_BUCKET_ID, TEST_DIR_ID,
        );
        let dir: Dir = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dir.id.as_str(), TEST_DIR_ID);
    }

    #[tokio::test]
    async fn test_get_user_dir_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs/{}",
            TEST_CLIENT_ID, TEST_BUCKET_ID, TEST_DIR_ID,
        );
        let dir: Dir = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dir.id.as_str(), TEST_DIR_ID);
    }

    #[tokio::test]
    async fn test_get_user_dir_not_found_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs/0196d28a2ca4792880b19b3a058d24b1",
            TEST_CLIENT_ID, TEST_BUCKET_ID,
        );
        let response = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .expect_failure()
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_list_user_files_as_user() {
        let server = create_test_app();
        let token = create_test_user_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs/{}/files",
            TEST_CLIENT_ID, TEST_BUCKET_ID, TEST_DIR_ID,
        );
        let dir: Paginated<FileDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dir.meta.total_records, 0);
    }

    #[tokio::test]
    async fn test_list_user_files_as_admin() {
        let server = create_test_app();
        let token = create_test_admin_auth_token().unwrap();
        let url = format!(
            "/clients/{}/buckets/{}/dirs/{}/files",
            TEST_CLIENT_ID, TEST_BUCKET_ID, TEST_DIR_ID,
        );
        let dir: Paginated<FileDto> = server
            .get(url.as_str())
            .authorization_bearer(token.as_str())
            .await
            .json();

        assert_eq!(dir.meta.total_records, 0);
    }
}
