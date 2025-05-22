use axum::Router;
use axum::extract::FromRef;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::Result;
use crate::config::Config;
use crate::web::all_routes;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
}

pub async fn run(config: Config) -> Result<()> {
    let port = config.port;
    let frontend_dir = config.frontend_dir.clone();
    let state = AppState {
        config: Arc::new(config),
    };

    let routes_all = Router::new()
        .merge(all_routes(state, &frontend_dir))
        .layer(CookieManagerLayer::new())
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
    info!("HTTP Server runnung on {}", addr);

    let listener = TcpListener::bind(addr).await.expect("Failed to bind");
    axum::serve(listener, routes_all.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server must start");

    info!("HTTP Server stopped");

    Ok(())
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
