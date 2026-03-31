use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use omni_backend::{
    services, Config, HistoricalDataService, PoolHistoricalService, PoolRealtimeService,
};

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
        "reset-partial" => {
            let protocol = args.get(2).cloned();
            let chain = args.get(3).cloned();
            if protocol.is_none() {
                eprintln!("Usage: omni-worker reset-partial <protocol> [chain]");
                eprintln!("  protocol: aave-v3, pancakeswap, aerodrome, uniswap-v3, ...");
                eprintln!("  chain (optional): ethereum, base, arbitrum, solana, ...");
                std::process::exit(1);
            }
            run_reset_partial(config, protocol.unwrap(), chain).await
        }
        "purge" => {
            let protocols: Vec<String> = args[2..].to_vec();
            if protocols.is_empty() {
                eprintln!("Usage: omni-worker purge <protocol1> <protocol2> ...");
                std::process::exit(1);
            }
            run_purge(config, &protocols).await
        }
        other => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: omni-worker [collect | reset | reset-partial <protocol> [chain] | purge <protocols...>]");
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
    let pool_historical_service =
        PoolHistoricalService::new(&config.mongodb_url, &config.mongodb_database).await?;
    let pool_realtime_service =
        PoolRealtimeService::new(&config.mongodb_url, &config.mongodb_database).await?;

    let aggregator = services::aggregator::RateAggregator::new(config.clone());

    let worker = services::DailyCollectionWorker::new(
        aggregator,
        historical_service,
        realtime_service,
        pool_historical_service,
        pool_realtime_service,
        config.backfill_days,
        config.backfill_concurrency,
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
    tracing::info!(
        "  Vaults with real history: {}",
        result.vaults_with_real_history
    );
    tracing::info!(
        "  Vaults skipped (no history): {}",
        result.vaults_skipped_no_history
    );
    tracing::info!("  Duration: {}s", result.duration_seconds);
    tracing::info!("  Skipped: {}", result.skipped);

    Ok(())
}

async fn run_reset_partial(
    config: Config,
    protocol: String,
    chain: Option<String>,
) -> anyhow::Result<()> {
    let label = match &chain {
        Some(c) => format!("{} on {}", protocol, c),
        None => protocol.clone(),
    };
    tracing::info!("Partial reset: clearing data for {}...", label);

    let db = mongodb::Client::with_uri_str(&config.mongodb_url)
        .await?
        .database(&config.mongodb_database);

    let mut filter = bson::doc! { "protocol": &protocol };
    if let Some(ref c) = chain {
        filter.insert("chain", c);
    }

    let snap = db
        .collection::<bson::Document>("rate_snapshots")
        .delete_many(filter.clone())
        .await?;
    tracing::info!("  Deleted {} from rate_snapshots", snap.deleted_count);

    let rt = db
        .collection::<bson::Document>("rate_realtime")
        .delete_many(filter.clone())
        .await?;
    tracing::info!("  Deleted {} from rate_realtime", rt.deleted_count);

    let pool_snap = db
        .collection::<bson::Document>("pool_snapshots")
        .delete_many(filter.clone())
        .await?;
    tracing::info!("  Deleted {} from pool_snapshots", pool_snap.deleted_count);

    let pool_rt = db
        .collection::<bson::Document>("pool_realtime")
        .delete_many(filter)
        .await?;
    tracing::info!("  Deleted {} from pool_realtime", pool_rt.deleted_count);

    let total =
        snap.deleted_count + rt.deleted_count + pool_snap.deleted_count + pool_rt.deleted_count;
    tracing::info!(
        "Partial reset complete: {} total documents deleted for {}",
        total,
        label
    );

    tracing::info!("Starting full data collection...");
    run_collection(config, false).await
}

async fn run_purge(config: Config, protocols: &[String]) -> anyhow::Result<()> {
    tracing::info!(
        "Purging data for {} protocols: {}",
        protocols.len(),
        protocols.join(", ")
    );

    let db = mongodb::Client::with_uri_str(&config.mongodb_url)
        .await?
        .database(&config.mongodb_database);

    let filter = bson::doc! { "protocol": { "$in": protocols } };

    let collections = [
        "rate_snapshots",
        "rate_realtime",
        "pool_snapshots",
        "pool_realtime",
    ];
    let mut total = 0u64;
    for coll_name in &collections {
        let result = db
            .collection::<bson::Document>(coll_name)
            .delete_many(filter.clone())
            .await?;
        tracing::info!("  Deleted {} from {}", result.deleted_count, coll_name);
        total += result.deleted_count;
    }

    tracing::info!("Purge complete: {} total documents deleted", total);
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

    tracing::info!("  Clearing pool_snapshots...");
    let pool_snapshots_result = db
        .collection::<bson::Document>("pool_snapshots")
        .delete_many(bson::doc! {})
        .await?;
    tracing::info!(
        "  Deleted {} documents from pool_snapshots",
        pool_snapshots_result.deleted_count
    );

    tracing::info!("  Clearing pool_realtime...");
    let pool_realtime_result = db
        .collection::<bson::Document>("pool_realtime")
        .delete_many(bson::doc! {})
        .await?;
    tracing::info!(
        "  Deleted {} documents from pool_realtime",
        pool_realtime_result.deleted_count
    );

    tracing::info!("Collections reset successfully!");
    tracing::info!("Starting data collection...");

    run_collection(config, false).await
}
