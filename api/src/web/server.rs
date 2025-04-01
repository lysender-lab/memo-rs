use axum::extract::FromRef;
use axum::{Router, middleware, response::Response};
use deadpool_diesel::sqlite::Pool;
use google_cloud_storage::client::Client;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, info};

use crate::Result;
use crate::config::Config;
use crate::db::create_db_pool;
use crate::error::ErrorInfo;
use crate::storage::create_storage_client;
use crate::web::routes::all_routes;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Config,
    pub storage_client: Client,
    pub db_pool: Pool,
}

pub async fn run_web_server(config: &Config) -> Result<()> {
    let port = config.server.port;

    let storage_client = create_storage_client(config.cloud.credentials.as_str()).await?;
    let pool = create_db_pool(config.db.url.as_str());
    let state = AppState {
        config: config.clone(),
        storage_client,
        db_pool: pool,
    };

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
    let status = res.status();
    if status.is_server_error() {
        let error = res.extensions().get::<ErrorInfo>();
        if let Some(e) = error {
            error!("{}", e.message);
            if let Some(bt) = &e.backtrace {
                error!("{}", bt);
            }
        }
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
