use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use omni_backend::{AppState, Config, HistoricalDataService, routes, services};

async fn create_app(config: Config) -> anyhow::Result<Router> {
    let historical_service =
        HistoricalDataService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let realtime_service =
        services::RealtimeService::new(&config.mongodb_url, &config.mongodb_database).await?;
    tracing::info!("Connected to MongoDB");

    let app_state = std::sync::Arc::new(AppState {
        config,
        historical_service,
        realtime_service,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/rates", get(routes::get_rates))
        .route("/api/v1/rates/history", get(routes::vault_history))
        .route("/api/v1/historical/backtest", get(routes::backtest))
        .route("/api/v1/assets", get(routes::get_assets))
        .layer(cors)
        .with_state(app_state);

    Ok(app)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,omni_backend=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;

    tracing::info!("Starting OMNI API on {}:{}", config.host, config.port);

    let app = create_app(config).await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Server listening on {}", addr);
    tracing::info!("API endpoint: http://{}/api/v1/rates", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
