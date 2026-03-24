use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use futures::stream::{self, StreamExt};
use crate::{
    models::RateQuery,
    services::{aggregator::RateAggregator, HistoricalDataService, HistoricalFetcher, RealtimeService, historical_fetcher::HistoricalDataPoint},
};

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
    backfill_days: i64,
}

impl DailyCollectionWorker {
    pub fn new(
        aggregator: RateAggregator,
        historical_service: HistoricalDataService,
        realtime_service: RealtimeService,
        backfill_days: i64,
        graph_api_key: Option<String>,
    ) -> Self {
        Self {
            aggregator,
            historical_service,
            historical_fetcher: HistoricalFetcher::new(graph_api_key),
            realtime_service,
            backfill_days,
        }
    }
    
    /// Execute daily collection
    /// 
    /// This function is idempotent: can be called multiple times 
    /// same day without duplicating data
    pub async fn collect(&self) -> Result<CollectionResult> {
        let start_time = Utc::now();
        tracing::info!("🔄 Starting daily collection worker");
        
        // Check if already collected today
        let already_collected = self.historical_service.has_snapshots_for_today().await?;
        if already_collected {
            tracing::info!("✅ Today's snapshot already exists, skipping collection");
            tracing::info!("📊 Consolidating rate_realtime collection...");
            let consolidated_count = self.realtime_service.consolidate_all().await?;
            tracing::info!("✅ Consolidated {} vaults into rate_realtime", consolidated_count);
            
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
        
        // Collect current rates for ALL vaults
        tracing::info!("📡 Fetching current rates from all protocols...");
        let query = RateQuery {
            action: None,       // All actions
            assets: None,        // All assets
            chains: None,       // All chains
            protocols: None,    // All protocols
            operation_types: None, // All operation types
            asset_categories: None, // All categories
            min_liquidity: 0,   // No minimum (get everything)
        };
        
        let rates = self.aggregator.get_rates(&query).await?;
        tracing::info!("✅ Fetched {} rates", rates.len());
        
        // Determine which vaults need backfill
        // For MVP: Always backfill all vaults to ensure historical data collection
        let mut vaults_needing_backfill = Vec::new();
        for rate in &rates {
            // Generate vault_id to check if it exists
            let vault_id = crate::models::RateSnapshot::generate_vault_id(
                &rate.protocol,
                &rate.chain,
                &rate.asset.to_string(),
                &rate.url,
                rate.operation_type,
                Some(&rate.action),
            );
            
            // For now: ALWAYS attempt backfill (APIs will return empty if no data)
            // This ensures we try to fetch historical data even if vault exists
            vaults_needing_backfill.push((vault_id.clone(), rate));
            
            let has_any_data = self.historical_service
                .get_latest_snapshot_date(&vault_id)
                .await?
                .is_some();
            
            if !has_any_data {
                tracing::info!(
                    "🆕 New vault: {} {} {} (will backfill {} days)",
                    rate.protocol,
                    rate.chain,
                    rate.asset.to_string(),
                    self.backfill_days
                );
            } else {
                tracing::debug!(
                    "🔄 Existing vault: {} {} {} (re-attempting backfill)",
                    rate.protocol,
                    rate.chain,
                    rate.asset.to_string()
                );
            }
        }
        
        // Perform backfill for new vaults
        let (backfills_performed, vaults_with_real_history) = if !vaults_needing_backfill.is_empty() {
            tracing::info!(
                "⏳ Backfilling {} vaults with {} days of data...",
                vaults_needing_backfill.len(),
                self.backfill_days
            );
            self.backfill_vaults(&vaults_needing_backfill).await?
        } else {
            (0, 0)
        };
        
        // Save today's snapshot
        tracing::info!("💾 Saving today's snapshot...");
        let today = Utc::now();

        // --- EVENT CHANGE LOGIC ---
        use crate::models::Protocol;
        let mut filtered_rates = Vec::with_capacity(rates.len());
        for rate in &rates {
            match rate.protocol {
                Protocol::Jupiter | Protocol::Jito | Protocol::RocketPool | Protocol::Kamino | Protocol::Fluid => {
                    // Busca o último snapshot desse vault
                    let vault_id = rate.vault_id.as_ref().cloned().unwrap_or_else(|| {
                        crate::models::RateSnapshot::generate_vault_id(
                            &rate.protocol,
                            &rate.chain,
                            &rate.asset.to_string(),
                            &rate.url,
                            rate.operation_type,
                            Some(&rate.action),
                        )
                    });
                    if let Ok(Some(last_snapshot_date)) = self.historical_service.get_latest_snapshot_date(&vault_id).await {
                        // Busca o último snapshot
                        if let Ok(mut history) = self.historical_service.query_history(crate::models::HistoricalQuery {
                            start_date: last_snapshot_date,
                            end_date: last_snapshot_date,
                            protocol: Some(rate.protocol.clone()),
                            chain: Some(rate.chain.clone()),
                            asset: Some(rate.asset.to_string()),
                            action: None,
                        }).await {
                            if let Some(last) = history.last() {
                                // Só salva se mudou o net_apy
                                if (last.net_apy - rate.net_apy).abs() > 1e-8 {
                                    filtered_rates.push(rate.clone());
                                }
                                // else: não mudou, não salva
                                continue;
                            }
                        }
                    }
                    // Se não tem snapshot anterior, salva
                    filtered_rates.push(rate.clone());
                }
                _ => {
                    filtered_rates.push(rate.clone());
                }
            }
        }

        let snapshots_saved = self.historical_service
            .save_snapshots_batch(&filtered_rates, today)
            .await?;
        
        // Consolidate rate_realtime collection with metrics
        tracing::info!("📊 Consolidating rate_realtime collection...");
        let consolidated_count = self.realtime_service.consolidate_all().await?;
        tracing::info!("✅ Consolidated {} vaults into rate_realtime", consolidated_count);
        
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
        
        // Save worker execution record
        let execution_record = crate::models::WorkerExecutionRecord {
            id: None,
            executed_at: Utc::now(),
            collection_date: today.format("%Y-%m-%d").to_string(),
            status: crate::models::ExecutionStatus::Success,
            error: None,
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
            protocol_breakdown: vec![],
            system_info: crate::models::SystemInfo {
                hostname: Some(std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string())),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                environment: std::env::var("ENVIRONMENT").ok(),
            },
        };
        
        if let Err(e) = self.historical_service.save_execution_record(&execution_record).await {
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
            min_liquidity: 0,
        };
        
        let rates = self.aggregator.get_rates(&query).await?;
        tracing::info!("✅ Fetched {} rates", rates.len());
        
        // Prepare all vaults for backfill
        let vaults_needing_backfill: Vec<_> = rates.iter()
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
        tracing::info!("   Vaults skipped (no history): {}", vaults_skipped_no_history);
        tracing::info!("   Duration: {}s", duration);
        
        Ok(CollectionResult {
            success: true,
            collected_date: Utc::now(),
            vaults_processed: 0,  // No daily collection
            snapshots_inserted: 0,  // No daily collection
            snapshots_updated: 0,  // No daily collection
            new_vaults_discovered: 0,  // No daily collection
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
        
        // Process vaults in parallel (10 concurrent tasks)
        let results: Vec<_> = stream::iter(vaults.iter())
            .map(|(vault_id, rate)| async move {
                self.backfill_single_vault(vault_id, rate).await
            })
            .buffer_unordered(10)
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
        
        tracing::info!("✅ Backfilled {} snapshots total across {} vaults", 
            backfilled_count, vaults_with_real_history);
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
        let historical_data = self.historical_fetcher
            .fetch_historical_data(
                &rate.protocol,
                &rate.chain,
                rate,
                backfill_start,
                today,
            )
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
                let existing_dates = self.historical_service
                    .get_existing_dates(vault_id, backfill_start, today)
                    .await?;
                
                // ✅ OPTIMIZATION 2: Save each rate change event (not daily aggregation)
                // DeFi protocols only emit events when rates change, so we preserve granularity
                use std::collections::HashSet;
                let existing_set: HashSet<i64> = existing_dates
                    .iter()
                    .map(|dt| dt.timestamp())
                    .collect();
                
                // Local deduplication: if subgraph returns multiple events with same timestamp,
                // keep only the LAST one (most recent value for that timestamp)
                let mut events_by_timestamp: std::collections::HashMap<i64, HistoricalDataPoint> = std::collections::HashMap::new();
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
                        point.date  // Use exact timestamp, not day_start
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
                    match self.historical_service
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
