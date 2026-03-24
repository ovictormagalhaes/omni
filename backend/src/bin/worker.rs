use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use omni_backend::{Config, HistoricalDataService, services};

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

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("collect");

    match command {
        "collect" => {
            let backfill_only = args.get(2).map(|s| s == "--backfill-only").unwrap_or(false);
            run_collection(config, backfill_only).await
        }
        "reset" => run_reset(config).await,
        other => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: omni-worker [collect [--backfill-only] | reset]");
            std::process::exit(1);
        }
    }
}

async fn run_collection(config: Config, backfill_only: bool) -> anyhow::Result<()> {
    if backfill_only {
        tracing::info!("Starting backfill-only mode");
    } else {
        tracing::info!("Starting collection worker");
    }

    let historical_service =
        HistoricalDataService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let realtime_service =
        services::RealtimeService::new(&config.mongodb_url, &config.mongodb_database).await?;

    let aggregator = services::aggregator::RateAggregator::new(config.clone());

    let worker = services::DailyCollectionWorker::new(
        aggregator,
        historical_service,
        realtime_service,
        config.backfill_days,
        config.the_graph_api_key.clone(),
    );

    let result = if backfill_only {
        worker.backfill_only().await?
    } else {
        worker.collect().await?
    };

    tracing::info!("Collection completed");
    tracing::info!("  Vaults processed: {}", result.vaults_processed);
    tracing::info!("  Snapshots inserted: {}", result.snapshots_inserted);
    tracing::info!("  Snapshots updated: {}", result.snapshots_updated);
    tracing::info!("  New vaults discovered: {}", result.new_vaults_discovered);
    tracing::info!("  Backfill snapshots: {}", result.backfill_snapshots);
    tracing::info!("  Vaults with real history: {}", result.vaults_with_real_history);
    tracing::info!("  Vaults skipped (no history): {}", result.vaults_skipped_no_history);
    tracing::info!("  Duration: {}s", result.duration_seconds);
    tracing::info!("  Skipped: {}", result.skipped);

    Ok(())
}

async fn run_reset(config: Config) -> anyhow::Result<()> {
    tracing::info!("Resetting MongoDB collections...");

    let db = mongodb::Client::with_uri_str(&config.mongodb_url)
        .await?
        .database(&config.mongodb_database);

    tracing::info!("  Clearing rate_snapshots...");
    let snapshots_result = db
        .collection::<bson::Document>("rate_snapshots")
        .delete_many(bson::doc! {})
        .await?;
    tracing::info!(
        "  Deleted {} documents from rate_snapshots",
        snapshots_result.deleted_count
    );

    tracing::info!("  Clearing rate_realtime...");
    let realtime_result = db
        .collection::<bson::Document>("rate_realtime")
        .delete_many(bson::doc! {})
        .await?;
    tracing::info!(
        "  Deleted {} documents from rate_realtime",
        realtime_result.deleted_count
    );

    tracing::info!("  Clearing worker_executions...");
    let worker_result = db
        .collection::<bson::Document>("worker_executions")
        .delete_many(bson::doc! {})
        .await?;
    tracing::info!(
        "  Deleted {} documents from worker_executions",
        worker_result.deleted_count
    );

    tracing::info!("Collections reset successfully!");
    tracing::info!("Starting data collection...");

    run_collection(config, false).await
}
