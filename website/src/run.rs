use axum::Router;
use axum::extract::FromRef;
use moka::sync::Cache;
use reqwest::{Client, ClientBuilder};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::Result;
use crate::config::Config;
use crate::web::all_routes;
use yaas::actor::Actor;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: Client,
    pub auth_cache: Cache<String, Actor>,
}

pub async fn run(config: Config) -> Result<()> {
    let server_address = config.server_address.clone();
    let frontend_dir = config.frontend_dir.clone();
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("HTTP Client is required");

    let auth_cache = Cache::builder()
        .time_to_live(Duration::from_secs(10 * 60))
        .time_to_idle(Duration::from_secs(60))
        .max_capacity(100)
        .build();

    let state = AppState {
        config: Arc::new(config),
        client,
        auth_cache,
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
    info!("HTTP Server runnung on {}", server_address);

    let listener = TcpListener::bind(server_address)
        .await
        .expect("Failed to bind");
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
