use std::sync::Arc;

use axum::Router;
use axum::extract::FromRef;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::Result;
use crate::config::Config;
use crate::web::{assets_routes, private_routes, public_routes, routes_fallback};

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
        .merge(private_routes(state.clone()))
        .merge(public_routes(state.clone()))
        .merge(assets_routes(&frontend_dir))
        .fallback_service(routes_fallback(state))
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
    info!("Listening on {}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, routes_all.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    // Wait for a signal to shut down
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for shutdown signal");

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
        signal::unix::signal(signal::unix::SignalKind::terminate())
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
