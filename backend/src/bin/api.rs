use axum::{routing::{get, post}, Router};
use std::net::SocketAddr;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use omni_backend::{AppState, Config, HistoricalDataService, PoolHistoricalService, PoolRealtimeService, routes, services};

async fn create_app(config: Config) -> anyhow::Result<Router> {
    let historical_service =
        HistoricalDataService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let realtime_service =
        services::RealtimeService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let pool_historical_service =
        PoolHistoricalService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let pool_realtime_service =
        PoolRealtimeService::new(&config.mongodb_url, &config.mongodb_database).await?;
    tracing::info!("Connected to MongoDB");

    let cors_origins = config.cors_origins.clone();

    let app_state = std::sync::Arc::new(AppState {
        config,
        historical_service,
        realtime_service,
        pool_historical_service,
        pool_realtime_service,
    });

    let origins: Vec<_> = cors_origins.iter()
        .filter_map(|o| o.parse::<axum::http::HeaderValue>().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any());

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/rates", get(routes::get_rates))
        .route("/api/v1/rates/history", get(routes::vault_history))
        .route("/api/v1/historical/backtest", get(routes::backtest))
        .route("/api/v1/pools", get(routes::get_pools))
        .route("/api/v1/pools/history", get(routes::pool_history))
        .route("/api/v1/pools/score", post(routes::score_pool))
        .route("/api/v1/lending/score", post(routes::score_lending))
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
