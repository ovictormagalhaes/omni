use crate::{
    models::{PoolCollectionStats, PoolQuery, PoolResult, PoolSnapshot, ProtocolStats, RateQuery},
    services::{
        aggregator::RateAggregator, historical_fetcher::HistoricalDataPoint,
        pool_historical_fetcher::PoolHistoricalFetcher, HistoricalDataService, HistoricalFetcher,
        PoolHistoricalService, PoolRealtimeService, RealtimeService,
    },
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;

/// Daily collection worker that gathers APY/APR data for backtesting
///
/// Runs once per day (in production via Kubernetes CronJob)
/// Behavior:
/// - Collects current rates from all protocols/chains/vaults
/// - Saves daily snapshot to MongoDB
/// - On first run for a vault: backfills last N days of historical data
pub struct DailyCollectionWorker {
    aggregator: RateAggregator,
    historical_service: HistoricalDataService,
    historical_fetcher: HistoricalFetcher,
    realtime_service: RealtimeService,
    pool_historical_service: PoolHistoricalService,
    pool_realtime_service: PoolRealtimeService,
    pool_historical_fetcher: PoolHistoricalFetcher,
    backfill_days: i64,
    backfill_concurrency: usize,
}

impl DailyCollectionWorker {
    pub fn new(
        aggregator: RateAggregator,
        historical_service: HistoricalDataService,
        realtime_service: RealtimeService,
        pool_historical_service: PoolHistoricalService,
        pool_realtime_service: PoolRealtimeService,
        backfill_days: i64,
        backfill_concurrency: usize,
        graph_api_key: Option<String>,
    ) -> Self {
        Self {
            aggregator,
            historical_service,
            historical_fetcher: HistoricalFetcher::new(graph_api_key.clone()),
            realtime_service,
            pool_historical_service,
            pool_realtime_service,
            pool_historical_fetcher: PoolHistoricalFetcher::new(graph_api_key),
            backfill_days,
            backfill_concurrency,
        }
    }

    /// Execute daily collection
    ///
    /// This function is idempotent: can be called multiple times
    /// same day without duplicating data
    pub async fn collect(&self) -> Result<CollectionResult> {
        let start_time = Utc::now();
        tracing::info!("🔄 Starting daily collection worker");

        // Check if a previous run already completed successfully today
        let already_collected = self
            .historical_service
            .has_successful_execution_for_today()
            .await?;
        if already_collected {
            tracing::info!("✅ Today's rate snapshot already exists, skipping rate collection");
            tracing::info!("📊 Consolidating rate_realtime collection...");
            let consolidated_count = self.realtime_service.consolidate_all().await?;
            tracing::info!(
                "✅ Consolidated {} vaults into rate_realtime",
                consolidated_count
            );

            // Always collect pools even if rates already collected today
            let pool_query = PoolQuery {
                asset_categories_0: None,
                asset_categories_1: None,
                token_a: None,
                token_b: None,
                token: None,
                pair: None,
                chains: None,
                protocols: None,
                pool_type: None,
                min_tvl: 0,
                min_volume: 0,
                normalized_pair: None,
                page: 1,
                page_size: 100,
            };
            match self.aggregator.get_pools(&pool_query).await {
                Ok(pools) => self.save_pools(&pools, Utc::now()).await,
                Err(e) => tracing::warn!("⚠️ Failed to fetch pools: {}", e),
            }

            return Ok(CollectionResult {
                success: true,
                collected_date: Utc::now(),
                vaults_processed: 0,
                snapshots_inserted: 0,
                snapshots_updated: 0,
                new_vaults_discovered: 0,
                backfill_snapshots: 0,
                vaults_with_real_history: 0,
                vaults_skipped_no_history: 0,
                duration_seconds: (Utc::now() - start_time).num_seconds(),
                skipped: true,
                error: None,
            });
        }

        // P0 OPTIMIZATION: Fetch rates AND pools in parallel
        tracing::info!("📡 Fetching rates + pools in parallel from all protocols...");
        let rate_query = RateQuery {
            action: None,
            assets: None,
            chains: None,
            protocols: None,
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 100,
        };
        let pool_query = PoolQuery {
            asset_categories_0: None,
            asset_categories_1: None,
            token_a: None,
            token_b: None,
            token: None,
            pair: None,
            chains: None,
            protocols: None,
            pool_type: None,
            min_tvl: 0,
            min_volume: 0,
            normalized_pair: None,
            page: 1,
            page_size: 100,
        };

        let (rates_result, pools_result) = tokio::join!(
            self.aggregator.get_rates_with_meta(&rate_query),
            self.aggregator.get_pools_with_meta(&pool_query),
        );

        let rates_output = rates_result?;
        let rates = rates_output.rates;
        let rate_task_meta = rates_output.task_meta;

        let (pools_fetched, pool_task_meta) = match pools_result {
            Ok(output) => (output.pools, output.task_meta),
            Err(e) => {
                tracing::warn!("⚠️ Failed to fetch pools in parallel: {}", e);
                (vec![], vec![])
            }
        };
        tracing::info!(
            "✅ Fetched {} rates + {} pools in parallel",
            rates.len(),
            pools_fetched.len()
        );

        // P1+P2: Smart backfill — batch check which vaults have data (1 query instead of N)
        let vault_ids: Vec<(String, &crate::models::RateResult)> = rates
            .iter()
            .map(|rate| {
                let vault_id = crate::models::RateSnapshot::generate_vault_id(
                    &rate.protocol,
                    &rate.chain,
                    &rate.asset.to_string(),
                    &rate.url,
                    rate.operation_type,
                    Some(&rate.action),
                );
                (vault_id, rate)
            })
            .collect();

        // P2: Batch fetch latest APY for ALL vaults (1 aggregation — skip unchanged rates)
        let event_change_vault_ids: Vec<String> =
            vault_ids.iter().map(|(vid, _)| vid.clone()).collect();

        let latest_apys = self
            .historical_service
            .get_latest_apys_batch(&event_change_vault_ids)
            .await
            .unwrap_or_default();

        // P1: Smart backfill — only backfill vaults WITHOUT any data
        let existing_vault_ids = self
            .historical_service
            .get_vaults_with_data(
                &vault_ids
                    .iter()
                    .map(|(v, _)| v.as_str())
                    .collect::<Vec<_>>(),
            )
            .await
            .unwrap_or_default();

        let vaults_needing_backfill: Vec<_> = vault_ids
            .iter()
            .filter(|(vid, _)| !existing_vault_ids.contains(vid))
            .map(|(vid, rate)| (vid.clone(), *rate))
            .collect();

        let new_vaults = vaults_needing_backfill.len();
        if new_vaults > 0 {
            tracing::info!(
                "🆕 {} new vaults need backfill (skipping {} existing)",
                new_vaults,
                vault_ids.len() - new_vaults
            );
        }

        let (backfills_performed, vaults_with_real_history) = if !vaults_needing_backfill.is_empty()
        {
            self.backfill_vaults(&vaults_needing_backfill).await?
        } else {
            (0, 0)
        };

        // Save today's snapshot with event-change filtering
        tracing::info!("💾 Saving today's snapshot...");
        let today = Utc::now();

        let mut filtered_rates = Vec::with_capacity(rates.len());
        let mut event_skipped = 0;
        for (vault_id, rate) in &vault_ids {
            // Skip unchanged rates for ALL protocols (event-change filtering)
            if let Some(&last_apy) = latest_apys.get(vault_id) {
                if (last_apy - rate.net_apy).abs() <= 1e-8 {
                    event_skipped += 1;
                    continue;
                }
            }
            filtered_rates.push((*rate).clone());
        }
        if event_skipped > 0 {
            tracing::info!(
                "Event-change: skipped {} unchanged rates, saving {}",
                event_skipped,
                filtered_rates.len()
            );
        }

        let snapshots_saved = self
            .historical_service
            .save_snapshots_batch(&filtered_rates, today)
            .await?;

        // Consolidate rate_realtime collection with metrics
        tracing::info!("📊 Consolidating rate_realtime collection...");
        let consolidated_count = self.realtime_service.consolidate_all().await?;
        tracing::info!(
            "✅ Consolidated {} vaults into rate_realtime",
            consolidated_count
        );

        // Save pool data (already fetched in parallel above)
        self.save_pools(&pools_fetched, today).await;

        let duration = (Utc::now() - start_time).num_seconds();

        let snapshots_inserted = rates.len();
        let snapshots_updated = 0;
        let new_vaults_discovered = 0;
        let vaults_skipped_no_history = 0;

        tracing::info!(
            "✅ Daily collection completed: {} vaults, {} snapshots, {} backfills, {}s",
            rates.len(),
            snapshots_inserted,
            backfills_performed,
            duration
        );

        // Build per-protocol+chain breakdown for rates
        // Count saved vaults per (protocol, chain)
        let mut saved_counts: HashMap<(crate::models::Protocol, crate::models::Chain), usize> =
            HashMap::new();
        for rate in &filtered_rates {
            *saved_counts
                .entry((rate.protocol.clone(), rate.chain.clone()))
                .or_default() += 1;
        }

        let protocol_breakdown: Vec<ProtocolStats> = rate_task_meta
            .iter()
            .map(|meta| {
                let vaults_saved = saved_counts
                    .get(&(meta.protocol.clone(), meta.chain.clone()))
                    .copied()
                    .unwrap_or(0);
                ProtocolStats {
                    protocol: meta.protocol.clone(),
                    chain: meta.chain.clone(),
                    vaults_found: meta.items_found,
                    vaults_saved,
                    execution_time_ms: meta.duration_ms,
                    error: meta.error.clone(),
                }
            })
            .collect();

        // Build per-protocol+chain breakdown for pools
        let pool_breakdown: Vec<PoolCollectionStats> = pool_task_meta
            .iter()
            .map(|meta| {
                PoolCollectionStats {
                    protocol: meta.protocol.clone(),
                    chain: meta.chain.clone(),
                    pools_found: meta.items_found,
                    pools_saved: meta.items_found, // all fetched pools are saved
                    execution_time_ms: meta.duration_ms,
                    error: meta.error.clone(),
                }
            })
            .collect();

        // Determine execution status
        let failed_rate_protocols: Vec<String> = rate_task_meta
            .iter()
            .filter(|m| m.error.is_some())
            .map(|m| format!("{:?}/{:?}", m.protocol, m.chain))
            .collect();
        let failed_pool_protocols: Vec<String> = pool_task_meta
            .iter()
            .filter(|m| m.error.is_some())
            .map(|m| format!("{:?}/{:?}", m.protocol, m.chain))
            .collect();
        let all_failed: Vec<String> = failed_rate_protocols
            .iter()
            .chain(&failed_pool_protocols)
            .cloned()
            .collect();

        let total_tasks = rate_task_meta.len() + pool_task_meta.len();
        let (status, exec_error) = if all_failed.is_empty() {
            (crate::models::ExecutionStatus::Success, None)
        } else if all_failed.len() < total_tasks {
            (
                crate::models::ExecutionStatus::PartialSuccess,
                Some(crate::models::ExecutionError {
                    message: format!("{} indexer(s) failed", all_failed.len()),
                    failed_protocols: all_failed,
                }),
            )
        } else {
            (
                crate::models::ExecutionStatus::Failed,
                Some(crate::models::ExecutionError {
                    message: "All indexers failed".to_string(),
                    failed_protocols: all_failed,
                }),
            )
        };

        // Save worker execution record
        let execution_record = crate::models::WorkerExecutionRecord {
            id: None,
            executed_at: Utc::now(),
            collection_date: today.format("%Y-%m-%d").to_string(),
            status,
            error: exec_error,
            stats: crate::models::ExecutionStats {
                vaults_processed: rates.len(),
                snapshots_inserted: snapshots_saved,
                snapshots_updated: 0,
                new_vaults_discovered: 0,
                backfill_snapshots_created: backfills_performed,
                vaults_with_real_history,
                vaults_skipped_no_history: 0,
                total_snapshots_in_db: None,
            },
            duration_seconds: duration,
            protocol_breakdown,
            pool_breakdown,
            system_info: crate::models::SystemInfo {
                hostname: Some(std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string())),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                environment: std::env::var("ENVIRONMENT").ok(),
            },
        };

        if let Err(e) = self
            .historical_service
            .save_execution_record(&execution_record)
            .await
        {
            tracing::warn!("Failed to save worker execution record: {}", e);
        }

        Ok(CollectionResult {
            success: true,
            collected_date: today,
            vaults_processed: rates.len(),
            snapshots_inserted,
            snapshots_updated,
            new_vaults_discovered,
            backfill_snapshots: backfills_performed,
            vaults_with_real_history,
            vaults_skipped_no_history,
            duration_seconds: duration,
            skipped: false,
            error: None,
        })
    }

    /// Backfill-only mode: just run backfill without daily collection
    /// Useful for re-running backfill after fixes or for testing
    /// Save pre-fetched pool data → upsert pool_realtime + save snapshots (event-changed)
    async fn save_pools(&self, pools: &[PoolResult], today: DateTime<Utc>) {
        if pools.is_empty() {
            tracing::info!("No pool data to save");
            return;
        }

        tracing::info!("💾 Saving {} pools to MongoDB...", pools.len());

        // P3: Run upsert + snapshots in parallel (independent writes to different collections)
        let pools_vec = pools.to_vec();
        let pool_realtime = self.pool_realtime_service.clone();
        let pool_historical = self.pool_historical_service.clone();

        let (upsert_result, snapshot_result) = tokio::join!(
            pool_realtime.upsert_from_results(&pools_vec),
            pool_historical.save_pool_snapshots(&pools_vec, today),
        );

        match upsert_result {
            Ok(count) => tracing::info!("💾 Upserted {} pools into pool_realtime", count),
            Err(e) => tracing::warn!("⚠️ Failed to upsert pool_realtime: {}", e),
        }
        match snapshot_result {
            Ok(saved) => tracing::info!("💾 Pool snapshots: {} saved (event-changed)", saved),
            Err(e) => tracing::warn!("⚠️ Failed to save pool snapshots: {}", e),
        }

        // P3: Run consolidation + backfill in parallel (both read snapshots, write to realtime)
        let pool_realtime2 = self.pool_realtime_service.clone();
        let (consolidate_result, backfill_result) =
            tokio::join!(pool_realtime2.consolidate_all(), self.backfill_pools(pools),);

        match consolidate_result {
            Ok(count) => tracing::info!("📊 Consolidated metrics for {} pools", count),
            Err(e) => tracing::warn!("⚠️ Failed to consolidate pool metrics: {}", e),
        }
        match backfill_result {
            Ok((backfilled, pools_with_history)) => {
                tracing::info!(
                    "📊 Pool backfill: {} snapshots across {} pools",
                    backfilled,
                    pools_with_history
                );
            }
            Err(e) => tracing::warn!("⚠️ Failed to backfill pool history: {}", e),
        }
    }

    // ========================================================================
    // POOL BACKFILL: Fetch historical poolDayData from The Graph
    // ========================================================================

    /// Backfill historical data for pools that lack sufficient snapshot history.
    async fn backfill_pools(&self, pools: &[PoolResult]) -> Result<(usize, usize)> {
        // Only backfill pools from protocols that support historical fetching
        let backfillable: Vec<&PoolResult> = pools
            .iter()
            .filter(|p| matches!(p.protocol, crate::models::Protocol::Uniswap))
            .collect();

        if backfillable.is_empty() {
            return Ok((0, 0));
        }

        let today = Utc::now();
        let backfill_start = today - Duration::days(self.backfill_days);
        let expected_days = self.backfill_days as usize;

        // Step 1: Batch check which pools already have enough history (1 aggregation vs N queries)
        let pool_vault_ids: Vec<&str> = backfillable
            .iter()
            .map(|p| p.pool_vault_id.as_str())
            .collect();

        let snapshot_counts = self
            .pool_historical_service
            .get_snapshot_counts_batch(&pool_vault_ids, backfill_start, today)
            .await
            .unwrap_or_default();

        let needs_backfill: Vec<&PoolResult> = backfillable
            .into_iter()
            .filter(|p| {
                let count = snapshot_counts.get(&p.pool_vault_id).copied().unwrap_or(0);
                count < (expected_days * 80 / 100)
            })
            .collect();

        if needs_backfill.is_empty() {
            tracing::info!(
                "Pool backfill: all {} pools already have >=80% history, skipping",
                pool_vault_ids.len()
            );
            return Ok((0, 0));
        }

        tracing::info!(
            "⏳ Pool backfill: {} pools need data (skipped {} with enough history)",
            needs_backfill.len(),
            pool_vault_ids.len() - needs_backfill.len()
        );

        // Step 2: Group pools by chain for batch GraphQL queries
        let mut by_chain: std::collections::HashMap<crate::models::Chain, Vec<&PoolResult>> =
            std::collections::HashMap::new();
        for pool in &needs_backfill {
            by_chain.entry(pool.chain.clone()).or_default().push(pool);
        }

        // Step 3: Batch fetch historical data per chain (1 query per ~30 pools instead of 1 per pool)
        let mut total_backfilled = 0;
        let mut pools_with_history = 0;

        for (chain, chain_pools) in &by_chain {
            let addresses: Vec<String> =
                chain_pools.iter().map(|p| p.pool_address.clone()).collect();

            let batch_data = match self
                .pool_historical_fetcher
                .fetch_pool_historical_batch(
                    &crate::models::Protocol::Uniswap,
                    chain,
                    &addresses,
                    backfill_start,
                    today,
                )
                .await
            {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("Pool backfill batch failed for {:?}: {:?}", chain, e);
                    continue;
                }
            };

            // Build lookup from pool_address → PoolResult
            let pool_by_addr: std::collections::HashMap<String, &&PoolResult> = chain_pools
                .iter()
                .map(|p| (p.pool_address.to_lowercase(), p))
                .collect();

            // Step 4: Convert data points to snapshots and batch insert
            let mut all_snapshots: Vec<PoolSnapshot> = Vec::new();

            for (pool_addr, data_points) in &batch_data {
                let pool = match pool_by_addr.get(pool_addr) {
                    Some(p) => *p,
                    None => continue,
                };

                let existing_count = snapshot_counts
                    .get(&pool.pool_vault_id)
                    .copied()
                    .unwrap_or(0);
                // Build existing dates set from count — we already know how many exist,
                // but not which dates. For simplicity, just insert and let the unique index dedup.
                let _ = existing_count;

                for point in data_points {
                    let day_start = point.date.date_naive().and_hms_opt(0, 0, 0).unwrap();
                    let day_dt = DateTime::from_naive_utc_and_offset(day_start, Utc);

                    let fee_apr_24h = if point.tvl_usd > 0.0 {
                        (point.fees_usd / point.tvl_usd) * 365.0 * 100.0
                    } else {
                        0.0
                    };

                    let turnover = if point.tvl_usd > 0.0 {
                        point.volume_usd / point.tvl_usd
                    } else {
                        0.0
                    };

                    all_snapshots.push(PoolSnapshot {
                        id: None,
                        date: day_dt,
                        pool_vault_id: pool.pool_vault_id.clone(),
                        protocol: pool.protocol.clone(),
                        chain: pool.chain.clone(),
                        token0: pool.token0.clone(),
                        token1: pool.token1.clone(),
                        pair: pool.pair.clone(),
                        normalized_pair: pool.normalized_pair.clone(),
                        pool_type: pool.pool_type.clone(),
                        fee_rate_bps: pool.fee_rate_bps,
                        tvl_usd: point.tvl_usd,
                        volume_24h_usd: point.volume_usd,
                        fees_24h_usd: point.fees_usd,
                        turnover_ratio_24h: turnover,
                        fee_apr_24h,
                        fee_apr_7d: fee_apr_24h,
                        rewards_apr: 0.0,
                        url: pool.url.clone(),
                        collected_at: Utc::now(),
                    });
                }

                pools_with_history += 1;
            }

            if !all_snapshots.is_empty() {
                let count = all_snapshots.len();
                match self
                    .pool_historical_service
                    .save_pool_snapshots_batch(all_snapshots)
                    .await
                {
                    Ok(inserted) => {
                        tracing::info!(
                            "✅ Pool backfill {:?}: saved {} snapshots for {} pools",
                            chain,
                            inserted,
                            batch_data.len()
                        );
                        total_backfilled += count;
                    }
                    Err(e) => {
                        tracing::warn!("Pool backfill batch insert failed for {:?}: {:?}", chain, e)
                    }
                }
            }
        }

        Ok((total_backfilled, pools_with_history))
    }

    pub async fn backfill_only(&self) -> Result<CollectionResult> {
        let start_time = Utc::now();
        tracing::info!("🔄 Starting backfill-only mode");

        // Fetch current rates to get all vaults
        tracing::info!("📡 Fetching current rates from all protocols...");
        let query = RateQuery {
            action: None,
            assets: None,
            chains: None,
            protocols: None,
            operation_types: None,
            asset_categories: None,
            token: None,
            min_liquidity: 0,
            page: 1,
            page_size: 100,
        };

        let rates = self.aggregator.get_rates(&query).await?;
        tracing::info!("✅ Fetched {} rates", rates.len());

        // Prepare all vaults for backfill
        let vaults_needing_backfill: Vec<_> = rates
            .iter()
            .map(|rate| {
                let vault_id = crate::models::RateSnapshot::generate_vault_id(
                    &rate.protocol,
                    &rate.chain,
                    &rate.asset.to_string(),
                    &rate.url,
                    rate.operation_type,
                    Some(&rate.action),
                );
                (vault_id, rate)
            })
            .collect();

        tracing::info!(
            "⏳ Backfilling {} vaults with {} days of data...",
            vaults_needing_backfill.len(),
            self.backfill_days
        );

        // Run backfill
        let (backfill_snapshots, vaults_with_real_history) =
            self.backfill_vaults(&vaults_needing_backfill).await?;

        let vaults_skipped_no_history = vaults_needing_backfill.len() - vaults_with_real_history;
        let duration = (Utc::now() - start_time).num_seconds();

        tracing::info!("✅ Backfill completed");
        tracing::info!("   Backfilled {} snapshots", backfill_snapshots);
        tracing::info!("   Vaults with real history: {}", vaults_with_real_history);
        tracing::info!(
            "   Vaults skipped (no history): {}",
            vaults_skipped_no_history
        );
        tracing::info!("   Duration: {}s", duration);

        Ok(CollectionResult {
            success: true,
            collected_date: Utc::now(),
            vaults_processed: 0,      // No daily collection
            snapshots_inserted: 0,    // No daily collection
            snapshots_updated: 0,     // No daily collection
            new_vaults_discovered: 0, // No daily collection
            backfill_snapshots,
            vaults_with_real_history,
            vaults_skipped_no_history,
            duration_seconds: duration,
            skipped: false,
            error: None,
        })
    }

    /// Backfill historical data for new vaults
    ///
    /// Strategy:
    /// 1. Try to fetch REAL historical data from TheGraph/APIs
    /// 2. If no real data: SKIP (no fallback simulation)
    ///
    /// Optimizations:
    /// - Batch verification with get_existing_dates() (1 query vs 1000)
    /// - Batch MongoDB inserts (1 insert vs 1000)
    /// - Parallel processing with buffer_unordered (10× speedup)
    async fn backfill_vaults(
        &self,
        vaults: &[(String, &crate::models::RateResult)],
    ) -> Result<(usize, usize)> {
        let mut backfilled_count = 0;
        let mut vaults_with_real_history = 0;

        // Process vaults in parallel
        let results: Vec<_> = stream::iter(vaults.iter())
            .map(|(vault_id, rate)| async move { self.backfill_single_vault(vault_id, rate).await })
            .buffer_unordered(self.backfill_concurrency)
            .collect()
            .await;

        // Aggregate results
        for result in results {
            match result {
                Ok((snapshots_saved, has_history)) => {
                    backfilled_count += snapshots_saved;
                    if has_history {
                        vaults_with_real_history += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to backfill vault: {:?}", e);
                }
            }
        }

        tracing::info!(
            "✅ Backfilled {} snapshots total across {} vaults",
            backfilled_count,
            vaults_with_real_history
        );
        Ok((backfilled_count, vaults_with_real_history))
    }

    /// Backfill a single vault (optimized with batch operations)
    async fn backfill_single_vault(
        &self,
        vault_id: &str,
        rate: &crate::models::RateResult,
    ) -> Result<(usize, bool)> {
        let today = Utc::now();
        let backfill_start = today - Duration::days(self.backfill_days);

        tracing::debug!(
            "📊 Fetching REAL historical data for vault {} ({} {} {}) from {} to {}",
            vault_id,
            rate.protocol,
            rate.chain,
            rate.asset,
            backfill_start.format("%Y-%m-%d"),
            today.format("%Y-%m-%d")
        );

        // Try to fetch real historical data
        let historical_data = self
            .historical_fetcher
            .fetch_historical_data(&rate.protocol, &rate.chain, rate, backfill_start, today)
            .await;

        match historical_data {
            Ok(data) if !data.is_empty() => {
                let data_len = data.len();

                tracing::info!(
                    "✅ Found {} real historical data points for vault {}",
                    data_len,
                    vault_id
                );

                // ✅ OPTIMIZATION 1: Batch fetch existing timestamps (1 query instead of 1000)
                let existing_dates = self
                    .historical_service
                    .get_existing_dates(vault_id, backfill_start, today)
                    .await?;

                // ✅ OPTIMIZATION 2: Save each rate change event (not daily aggregation)
                // DeFi protocols only emit events when rates change, so we preserve granularity
                use std::collections::HashSet;
                let existing_set: HashSet<i64> =
                    existing_dates.iter().map(|dt| dt.timestamp()).collect();

                // Local deduplication: if subgraph returns multiple events with same timestamp,
                // keep only the LAST one (most recent value for that timestamp)
                let mut events_by_timestamp: std::collections::HashMap<i64, HistoricalDataPoint> =
                    std::collections::HashMap::new();
                for point in data {
                    let ts = point.date.timestamp();
                    // HashMap insert replaces previous value, so last point wins
                    events_by_timestamp.insert(ts, point);
                }

                let mut snapshots_to_insert = Vec::new();

                for (_ts, point) in events_by_timestamp {
                    let event_timestamp = point.date.timestamp();

                    // Skip if this exact timestamp already exists in DB
                    if existing_set.contains(&event_timestamp) {
                        continue;
                    }

                    // Create snapshot with exact timestamp
                    let historical_rate = crate::models::RateResult {
                        protocol: rate.protocol.clone(),
                        chain: rate.chain.clone(),
                        asset: rate.asset.clone(),
                        action: rate.action.clone(),
                        asset_category: rate.asset_category.clone(),
                        apy: point.supply_apy,
                        rewards: rate.rewards,
                        net_apy: point.supply_apy + rate.rewards,
                        performance_fee: rate.performance_fee,
                        active: rate.active,
                        collateral_enabled: rate.collateral_enabled,
                        collateral_ltv: rate.collateral_ltv,
                        liquidity: point.available_liquidity,
                        total_liquidity: point.total_liquidity,
                        utilization_rate: point.utilization_rate,
                        operation_type: rate.operation_type,
                        url: rate.url.clone(),
                        vault_id: rate.vault_id.clone(),
                        vault_name: rate.vault_name.clone(),
                        last_update: point.date,
                        apy_metrics: None, // Historical snapshots don't have APY metrics
                    };

                    let snapshot = crate::models::RateSnapshot::from_rate_result(
                        &historical_rate,
                        point.date, // Use exact timestamp, not day_start
                    );
                    snapshots_to_insert.push(snapshot);
                }

                tracing::debug!(
                    "Found {} rate change events, {} new events to save for vault {}",
                    data_len,
                    snapshots_to_insert.len(),
                    vault_id
                );

                // ✅ OPTIMIZATION 3: Batch insert all snapshots at once
                let saved_count = if !snapshots_to_insert.is_empty() {
                    match self
                        .historical_service
                        .save_snapshots_batch_optimized(snapshots_to_insert)
                        .await
                    {
                        Ok(count) => {
                            tracing::info!(
                                "✅ Saved {} new snapshots for vault {}",
                                count,
                                vault_id
                            );
                            count
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to batch save snapshots for vault {}: {:?}",
                                vault_id,
                                e
                            );
                            0
                        }
                    }
                } else {
                    tracing::debug!("No new snapshots to save for vault {}", vault_id);
                    0
                };

                Ok((saved_count, true))
            }
            _ => {
                // NO FALLBACK: Skip vault without real historical data
                tracing::debug!(
                    "⏭️  Skipping {} {} {} - No real historical data available",
                    rate.protocol,
                    rate.chain,
                    rate.asset
                );
                Ok((0, false))
            }
        }
    }
}

/// Result of collection run
#[derive(Debug, serde::Serialize)]
pub struct CollectionResult {
    pub success: bool,
    pub collected_date: DateTime<Utc>,
    pub vaults_processed: usize,
    pub snapshots_inserted: usize,
    pub snapshots_updated: usize,
    pub new_vaults_discovered: usize,
    pub backfill_snapshots: usize,
    pub vaults_with_real_history: usize,
    pub vaults_skipped_no_history: usize,
    pub duration_seconds: i64,
    pub skipped: bool,
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "collection_worker_test.rs"]
mod tests;
