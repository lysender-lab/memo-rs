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
        .expect_success_by_default()
        .mock_transport()
        .build(app)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
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
                "username": "root",
                "password": "not-a-strong-password",
            }))
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
}
