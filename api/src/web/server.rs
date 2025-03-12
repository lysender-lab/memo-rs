use std::sync::Arc;

use axum::Router;
use axum::extract::FromRef;
use deadpool_diesel::sqlite::Pool;
use google_cloud_storage::client::Client;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::Result;
use crate::config::Config;
use crate::db::create_db_pool;
use crate::storage::create_storage_client;
use crate::web::routes::all_routes;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    pub storage_client: Arc<Client>,
    pub db_pool: Pool,
}

pub async fn run_web_server(config: &Config) -> Result<()> {
    let port = config.server.port;

    let storage_client = create_storage_client(config.cloud.credentials.as_str()).await?;
    let pool = create_db_pool(config.db.url.as_str());
    let state = AppState {
        config: Arc::new(config.clone()),
        storage_client: Arc::new(storage_client),
        db_pool: pool,
    };

    let mut routes_all = Router::new().merge(all_routes(state));

    routes_all = routes_all.layer(
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
        .await
        .unwrap();

    Ok(())
}
