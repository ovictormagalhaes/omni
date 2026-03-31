use crate::models::{
    BacktestStats, HistoricalQuery, Protocol, RateResult, RateSnapshot, WorkerExecutionRecord,
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use futures::stream::StreamExt;
use mongodb::bson::doc;
use mongodb::{Client, Collection, Database};
use std::collections::HashSet;

#[derive(Clone)]
pub struct HistoricalDataService {
    db: Database,
    collection: Collection<RateSnapshot>,
    execution_records: Collection<WorkerExecutionRecord>,
}

impl HistoricalDataService {
    /// Create new historical data service
    pub async fn new(mongodb_url: &str, database: &str) -> Result<Self> {
        let client = Client::with_uri_str(mongodb_url).await?;
        let db: Database = client.database(database);
        let collection: Collection<RateSnapshot> = db.collection("rate_snapshots");
        let execution_records: Collection<WorkerExecutionRecord> =
            db.collection("worker_executions");

        // Auto-migration: delete documents where `date` is stored as a String instead of
        // BSON DateTime.  This can happen after upgrading the serde helper annotation.
        // Safe to run on every startup — skips silently when no stale docs exist.
        {
            let raw: Collection<mongodb::bson::Document> = db.collection("rate_snapshots");
            let result = raw
                .delete_many(doc! { "date": { "$type": "string" } })
                .await;
            match result {
                Ok(r) if r.deleted_count > 0 => tracing::warn!(
                    "⚠️  Auto-migration: removed {} legacy rate_snapshots \
                     with string-typed date field (schema upgrade to BSON DateTime)",
                    r.deleted_count
                ),
                Ok(_) => {}
                Err(e) => tracing::warn!("Auto-migration check failed (non-fatal): {}", e),
            }
        }

        // Create indexes for efficient queries
        Self::create_indexes(&collection).await?;
        Self::create_execution_indexes(&execution_records).await?;

        Ok(Self {
            db,
            collection,
            execution_records,
        })
    }

    /// Create database indexes for query optimization
    async fn create_indexes(collection: &Collection<RateSnapshot>) -> Result<()> {
        use mongodb::bson::doc;
        use mongodb::options::IndexOptions;
        use mongodb::IndexModel;

        // UNIQUE constraint: one snapshot per (vault_id, date).
        // vault_id already encodes protocol+chain+asset+url+operation_type via SHA-256,
        // so this single compound index is sufficient to prevent all duplicates.
        // The extra `operation_type` field is included as belt-and-suspenders in case
        // two different vault_ids ever collide (statistically impossible at our scale,
        // but costs nothing to guard against).
        let unique_opts = IndexOptions::builder().unique(true).build();
        let index_unique = IndexModel::builder()
            .keys(doc! {
                "vault_id":       1,
                "date":           1,
                "operation_type": 1,
            })
            .options(unique_opts)
            .build();

        // Index for date range queries
        let index_date = IndexModel::builder().keys(doc! { "date": -1 }).build();

        // Index for protocol/chain queries
        let index_protocol = IndexModel::builder()
            .keys(doc! {
                "protocol": 1,
                "chain":    1,
                "date":     -1
            })
            .build();

        collection
            .create_indexes(vec![index_unique, index_date, index_protocol])
            .await?;

        tracing::info!("MongoDB indexes created for rate_snapshots collection");
        Ok(())
    }

    /// Save a daily snapshot from current rate — idempotent via replace_one upsert.
    /// The unique index on (vault_id, date) ensures only one document per vault per day.
    pub async fn save_snapshot(&self, rate: &RateResult, date: DateTime<Utc>) -> Result<()> {
        let snapshot = RateSnapshot::from_rate_result(rate, Self::get_day_start(date));

        // Filter matches the unique key (mirrors the unique index fields)
        let filter = doc! {
            "vault_id":       &snapshot.vault_id,
            "date":           snapshot.date,
            "operation_type": bson::to_bson(&snapshot.operation_type)?,
        };

        // replace_one: atomically swap the whole document; upsert inserts if missing.
        // snapshot.id is None so no _id is sent in the replacement — MongoDB keeps the
        // existing _id on update or mints a new one on insert.
        let mut options = mongodb::options::ReplaceOptions::default();
        options.upsert = Some(true);

        match self
            .collection
            .replace_one(filter, &snapshot)
            .with_options(options)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                // E11000: duplicate key — another concurrent task already wrote this
                // snapshot. Treat as success (idempotent).
                if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
                    ref we,
                )) = *e.kind
                {
                    if we.code == 11000 {
                        tracing::debug!(
                            "Snapshot already exists (race condition, safe to ignore): \
                             vault_id={} date={}",
                            snapshot.vault_id,
                            snapshot.date,
                        );
                        return Ok(());
                    }
                }
                return Err(e.into());
            }
        }

        tracing::debug!(
            "Saved snapshot (upsert): vault_id={} {} {} {} date={}",
            snapshot.vault_id,
            snapshot.protocol,
            snapshot.chain,
            snapshot.asset,
            snapshot.date
        );

        Ok(())
    }

    /// Save multiple snapshots in batch using parallel upserts with bounded concurrency.
    /// Each snapshot is upserted (idempotent via unique index on vault_id+date+operation_type).
    pub async fn save_snapshots_batch(
        &self,
        rates: &[RateResult],
        date: DateTime<Utc>,
    ) -> Result<usize> {
        if rates.is_empty() {
            return Ok(0);
        }

        let day_start = Self::get_day_start(date);
        let collection = self.collection.clone();

        let upsert_futures: Vec<_> = rates
            .iter()
            .map(|rate| {
                let snapshot = RateSnapshot::from_rate_result(rate, day_start);
                let coll = collection.clone();
                async move {
                    let filter = doc! {
                        "vault_id":       &snapshot.vault_id,
                        "date":           snapshot.date,
                        "operation_type": mongodb::bson::to_bson(&snapshot.operation_type)?,
                    };
                    let mut options = mongodb::options::ReplaceOptions::default();
                    options.upsert = Some(true);
                    match coll
                        .replace_one(filter, &snapshot)
                        .with_options(options)
                        .await
                    {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            // E11000 duplicate key = race condition, treat as success
                            if let mongodb::error::ErrorKind::Write(
                                mongodb::error::WriteFailure::WriteError(ref we),
                            ) = *e.kind
                            {
                                if we.code == 11000 {
                                    return Ok(());
                                }
                            }
                            Err(anyhow::anyhow!(e))
                        }
                    }
                }
            })
            .collect();

        let total = upsert_futures.len();
        let stream = futures::stream::iter(upsert_futures).buffer_unordered(20);
        futures::pin_mut!(stream);

        let mut saved_count = 0;
        while let Some(result) = stream.next().await {
            match result {
                Ok(()) => saved_count += 1,
                Err(e) => tracing::warn!("Failed to save snapshot: {:?}", e),
            }
        }

        tracing::info!(
            "Saved {}/{} snapshots for date {}",
            saved_count,
            total,
            date
        );
        Ok(saved_count)
    }

    /// Check if snapshot exists for a specific vault and date
    pub async fn has_snapshot(&self, vault_id: &str, date: DateTime<Utc>) -> Result<bool> {
        let day_start = Self::get_day_start(date);

        let filter = doc! {
            "vault_id": vault_id,
            "date": day_start,
        };

        let count = self.collection.count_documents(filter).await?;
        Ok(count > 0)
    }

    /// Get the latest snapshot date for a vault
    /// P2: Batch check which vault_ids have any snapshot data (1 query instead of N).
    pub async fn get_vaults_with_data(
        &self,
        vault_ids: &[&str],
    ) -> Result<std::collections::HashSet<String>> {
        if vault_ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let ids: Vec<String> = self
            .collection
            .distinct("vault_id", doc! { "vault_id": { "$in": vault_ids } })
            .await?
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        Ok(ids.into_iter().collect())
    }

    /// P2: Batch fetch latest net_apy for multiple vaults (1 aggregation instead of N*2 queries).
    pub async fn get_latest_apys_batch(
        &self,
        vault_ids: &[String],
    ) -> Result<std::collections::HashMap<String, f64>> {
        if vault_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let pipeline = vec![
            doc! { "$match": { "vault_id": { "$in": vault_ids } } },
            doc! { "$sort": { "date": -1 } },
            doc! { "$group": {
                "_id": "$vault_id",
                "latest_apy": { "$first": "$net_apy" },
            }},
        ];

        let db = &self.db;
        let mut cursor = db
            .collection::<mongodb::bson::Document>("rate_snapshots")
            .aggregate(pipeline)
            .await?;

        let mut result = std::collections::HashMap::new();
        while let Some(Ok(doc)) = cursor.next().await {
            if let (Ok(id), Ok(apy)) = (doc.get_str("_id"), doc.get_f64("latest_apy")) {
                result.insert(id.to_string(), apy);
            }
        }

        tracing::debug!("Batch fetched latest APY for {} vaults", result.len());
        Ok(result)
    }

    pub async fn get_latest_snapshot_date(&self, vault_id: &str) -> Result<Option<DateTime<Utc>>> {
        let filter = doc! {
            "vault_id": vault_id,
        };

        let options = mongodb::options::FindOneOptions::builder()
            .sort(doc! { "date": -1 })
            .build();

        if let Some(snapshot) = self
            .collection
            .find_one(filter)
            .with_options(options)
            .await?
        {
            Ok(Some(snapshot.date))
        } else {
            Ok(None)
        }
    }

    /// Get all existing snapshot timestamps for a vault within a date range (batch operation)
    /// This is 1000× faster than calling has_snapshot() in a loop.
    /// Returns exact timestamps, not day-start truncated dates.
    pub async fn get_existing_dates(
        &self,
        vault_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<HashSet<DateTime<Utc>>> {
        let filter = doc! {
            "vault_id": vault_id,
            "date": {
                "$gte": start,
                "$lte": end,
            }
        };

        let projection = doc! { "date": 1, "_id": 0 };
        let options = mongodb::options::FindOptions::builder()
            .projection(projection)
            .build();

        let mut cursor = self.collection.find(filter).with_options(options).await?;
        let mut dates = HashSet::new();

        while let Some(result) = cursor.next().await {
            if let Ok(snapshot) = result {
                dates.insert(snapshot.date);
            }
        }

        Ok(dates)
    }

    /// Save multiple snapshots in a true batch (optimized with insert_many)
    pub async fn save_snapshots_batch_optimized(
        &self,
        snapshots: Vec<RateSnapshot>,
    ) -> Result<usize> {
        if snapshots.is_empty() {
            return Ok(0);
        }

        let _count = snapshots.len();

        // Use ordered=false to continue on duplicate key errors
        let options = mongodb::options::InsertManyOptions::builder()
            .ordered(false)
            .build();

        match self
            .collection
            .insert_many(snapshots)
            .with_options(options)
            .await
        {
            Ok(result) => {
                tracing::debug!("Batch inserted {} snapshots", result.inserted_ids.len());
                Ok(result.inserted_ids.len())
            }
            Err(e) => {
                // If we get bulk write errors, some might have succeeded
                // Check if it's a bulk write failure (some inserts may have succeeded)
                if let mongodb::error::ErrorKind::BulkWrite(ref _bw) = *e.kind {
                    // On duplicate key errors, still return success for what was inserted
                    // MongoDB doesn't provide exact count, so we estimate
                    tracing::debug!("Batch insert had some duplicates (expected, ignored)");
                    // Return 0 since we can't determine exact count without more complex tracking
                    return Ok(0);
                }
                Err(e.into())
            }
        }
    }

    /// Check if a successful worker execution record exists for today.
    /// Only returns true when the full collection completed without errors,
    /// so partial runs (crash, timeout) are safely retried on re-execution.
    pub async fn has_successful_execution_for_today(&self) -> Result<bool> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let filter = doc! {
            "collectionDate": &today,
            "status": "success",
        };
        let count = self.execution_records.count_documents(filter).await?;
        Ok(count > 0)
    }

    /// Query historical data with filters
    pub async fn query_history(&self, query: HistoricalQuery) -> Result<Vec<RateSnapshot>> {
        let mut filter = doc! {
            "date": {
                "$gte": query.start_date,
                "$lte": query.end_date,
            }
        };

        if let Some(protocol) = query.protocol {
            filter.insert("protocol", bson::to_bson(&protocol)?);
        }

        if let Some(chain) = query.chain {
            filter.insert("chain", bson::to_bson(&chain)?);
        }

        if let Some(asset) = query.asset {
            filter.insert("asset", asset);
        }

        let mut cursor = self
            .collection
            .find(filter)
            .sort(doc! { "date": 1 })
            .await?;

        let mut results = Vec::new();
        while let Some(snapshot) = cursor.next().await {
            results.push(snapshot?);
        }

        tracing::info!("Retrieved {} historical snapshots", results.len());
        Ok(results)
    }

    /// Calculate backtest statistics for a given period
    pub async fn backtest(&self, query: HistoricalQuery) -> Result<BacktestStats> {
        let snapshots = self.query_history(query.clone()).await?;

        if snapshots.is_empty() {
            anyhow::bail!("No historical data found for query");
        }

        // Use net_apy for all calculations
        let rates: Vec<f64> = snapshots.iter().map(|s| s.net_apy).collect();

        if rates.is_empty() {
            anyhow::bail!("No valid rate data in historical snapshots");
        }

        // Calculate statistics
        let sum: f64 = rates.iter().sum();
        let avg_apy = sum / rates.len() as f64;
        let min_apy = rates.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_apy = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Standard deviation
        let variance: f64 =
            rates.iter().map(|r| (r - avg_apy).powi(2)).sum::<f64>() / rates.len() as f64;
        let std_deviation = variance.sqrt();

        // Find best protocol by average APY
        let mut protocol_rates: std::collections::HashMap<String, Vec<f64>> =
            std::collections::HashMap::new();

        for snapshot in &snapshots {
            protocol_rates
                .entry(format!("{:?}", snapshot.protocol))
                .or_default()
                .push(snapshot.net_apy);
        }

        let (best_protocol_name, best_protocol_avg_apy) = protocol_rates
            .iter()
            .map(|(protocol, rates)| {
                let avg = rates.iter().sum::<f64>() / rates.len() as f64;
                (protocol.clone(), avg)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or(("Unknown".to_string(), 0.0));

        // Calculate hypothetical earnings on $1M
        let days = (query.end_date - query.start_date).num_days();
        let years = days as f64 / 365.0;
        let earnings_on_1m = 1_000_000.0 * (avg_apy / 100.0) * years;

        let asset = query.asset.clone().unwrap_or_else(|| "USDC".to_string());

        // Parse protocol name back from string
        let best_protocol = match best_protocol_name.as_str() {
            "Aave" => Protocol::Aave,
            "Kamino" => Protocol::Kamino,
            "Morpho" => Protocol::Morpho,
            "Fluid" => Protocol::Fluid,
            "SparkLend" => Protocol::SparkLend,
            "JustLend" => Protocol::JustLend,
            "Euler" => Protocol::Euler,
            "Jupiter" => Protocol::Jupiter,
            "Lido" => Protocol::Lido,
            "Marinade" => Protocol::Marinade,
            "Jito" => Protocol::Jito,
            "RocketPool" => Protocol::RocketPool,
            _ => Protocol::Aave, // Fallback
        };

        Ok(BacktestStats {
            asset,
            period_start: query.start_date,
            period_end: query.end_date,
            avg_apy,
            min_apy,
            max_apy,
            std_deviation,
            best_protocol,
            best_protocol_avg_apy,
            earnings_on_1m,
            sample_size: rates.len(),
        })
    }

    /// Get the start of the day (00:00:00 UTC) for a given datetime
    pub fn get_day_start(dt: DateTime<Utc>) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0)
            .single()
            .unwrap_or(dt)
    }

    // ========== Vault Detail / History ==========

    /// Return time-series APY data with daily granularity for the last 90 days.
    ///
    /// BEHAVIOR:
    /// - Always returns 90 daily data points (one per day)
    /// - Groups multiple rate changes on the same day → takes LAST value of day
    /// - Forward-fills missing days with last known value
    /// - If no data in last 90 days, uses most recent historical snapshot
    /// - Frontend filters locally for 30/60/90 day views
    pub async fn get_vault_history(
        &self,
        vault_id: Option<&str>,
        protocol: Option<&crate::models::Protocol>,
        chain: Option<&crate::models::Chain>,
        asset: Option<&str>,
    ) -> Result<crate::models::VaultHistoryResponse> {
        use crate::models::VaultHistoryPoint;
        use chrono::Duration;

        // Always fetch 90 days for frontend flexibility
        let days_to_fetch = 90i64;
        let end = Self::get_day_start(Utc::now()) + Duration::days(1);
        let start = end - Duration::days(days_to_fetch);

        // Step 1: Build filter
        let mut base_filter = doc! {};
        if let Some(vid) = vault_id {
            base_filter.insert("vault_id", vid);
        } else {
            if let Some(p) = protocol {
                base_filter.insert("protocol", bson::to_bson(p)?);
            }
            if let Some(c) = chain {
                base_filter.insert("chain", bson::to_bson(c)?);
            }
            if let Some(a) = asset {
                let regex = mongodb::bson::Regex {
                    pattern: format!("^{}$", regex::escape(a)),
                    options: "i".to_string(),
                };
                base_filter.insert("asset", bson::Bson::RegularExpression(regex));
            }
        }

        tracing::info!(
            "vault_history: vault_id={:?} protocol={:?} chain={:?} asset={:?} fetching 90 days",
            vault_id,
            protocol,
            chain,
            asset
        );

        // Step 2: Use MongoDB aggregation to group by day
        // This takes multiple rate changes per day and keeps the LAST one
        let pipeline = vec![
            // Filter by vault + date range
            doc! {
                "$match": {
                    "$and": [
                        base_filter.clone(),
                        doc! {
                            "date": {
                                "$gte": start,
                                "$lt": end,
                            }
                        }
                    ]
                }
            },
            // Sort by date ascending (so $last picks most recent value of day)
            doc! { "$sort": { "date": 1 } },
            // Group by day (truncate to start of day)
            doc! {
                "$group": {
                    "_id": {
                        "$dateToString": {
                            "format": "%Y-%m-%d",
                            "date": "$date"
                        }
                    },
                    "date": { "$last": "$date" },
                    "net_apy": { "$last": "$net_apy" },
                    "base_apy": { "$last": "$base_apy" },
                    "rewards_apy": { "$last": "$rewards_apy" },
                    "liquidity_usd": { "$last": "$liquidity_usd" },
                    "utilization_rate": { "$last": "$utilization_rate" },
                    "vault_id": { "$first": "$vault_id" },
                    "vault_name": { "$first": "$vault_name" },
                    "protocol": { "$first": "$protocol" },
                    "chain": { "$first": "$chain" },
                    "asset": { "$first": "$asset" },
                    "operation_type": { "$first": "$operation_type" },
                    "url": { "$first": "$url" },
                }
            },
            // Sort by date again
            doc! { "$sort": { "date": 1 } },
        ];

        let mut cursor = self.collection.aggregate(pipeline).await?;
        let mut raw_points = Vec::new();
        let mut meta_vault_id = vault_id.map(|s| s.to_string());
        let mut meta_vault_name: Option<String> = None;
        let mut meta_protocol: Option<crate::models::Protocol> = None;
        let mut meta_chain: Option<crate::models::Chain> = None;
        let mut meta_asset: Option<String> = None;
        let mut meta_op_type: Option<crate::models::OperationType> = None;
        let mut meta_url: Option<String> = None;

        while let Some(result) = cursor.next().await {
            let doc = result?;
            if let (
                Some(date),
                Some(net_apy),
                Some(base_apy),
                Some(rewards_apy),
                Some(liquidity),
                Some(utilization),
            ) = (
                doc.get_datetime("date").ok(),
                doc.get_f64("net_apy").ok(),
                doc.get_f64("base_apy").ok(),
                doc.get_f64("rewards_apy").ok(),
                doc.get_i64("liquidity_usd")
                    .ok()
                    .or_else(|| doc.get_i32("liquidity_usd").ok().map(|v| v as i64)),
                doc.get_i32("utilization_rate").ok(),
            ) {
                if meta_vault_id.is_none() {
                    meta_vault_id = doc.get_str("vault_id").ok().map(|s| s.to_string());
                }
                if meta_vault_name.is_none() {
                    meta_vault_name = doc.get_str("vault_name").ok().map(|s| s.to_string());
                }
                if meta_protocol.is_none() {
                    if let Ok(p_str) = doc.get_str("protocol") {
                        meta_protocol = bson::from_bson(bson::Bson::String(p_str.to_string())).ok();
                    }
                }
                if meta_chain.is_none() {
                    if let Ok(c_str) = doc.get_str("chain") {
                        meta_chain = bson::from_bson(bson::Bson::String(c_str.to_string())).ok();
                    }
                }
                if meta_asset.is_none() {
                    meta_asset = doc.get_str("asset").ok().map(|s| s.to_string());
                }
                if meta_op_type.is_none() {
                    if let Ok(op_str) = doc.get_str("operation_type") {
                        meta_op_type = bson::from_bson(bson::Bson::String(op_str.to_string())).ok();
                    }
                }
                if meta_url.is_none() {
                    meta_url = doc.get_str("url").ok().map(|s| s.to_string());
                }

                raw_points.push(VaultHistoryPoint {
                    date: date.to_chrono(),
                    net_apy,
                    base_apy,
                    rewards_apy,
                    liquidity_usd: liquidity as u64,
                    utilization_rate: utilization as u32,
                });
            }
        }

        // Step 3: Handle case where no data in last 90 days
        // → Find most recent snapshot (could be 6 months old)
        if raw_points.is_empty() {
            tracing::warn!("No data in last 90 days, searching for most recent snapshot");

            let mut fallback_filter = base_filter.clone();
            fallback_filter.insert("date", doc! { "$lt": end });

            if let Some(snapshot) = self
                .collection
                .find_one(fallback_filter)
                .sort(doc! { "date": -1 })
                .await?
            {
                tracing::info!("Found fallback snapshot from {}", snapshot.date);
                meta_vault_id = vault_id
                    .map(|s| s.to_string())
                    .or(Some(snapshot.vault_id.clone()));
                meta_vault_name = snapshot.vault_name.clone();
                meta_protocol = Some(snapshot.protocol.clone());
                meta_chain = Some(snapshot.chain.clone());
                meta_asset = Some(snapshot.asset.clone());
                meta_op_type = Some(snapshot.operation_type);
                meta_url = Some(snapshot.url.clone());

                raw_points.push(VaultHistoryPoint {
                    date: snapshot.date,
                    net_apy: snapshot.net_apy,
                    base_apy: snapshot.base_apy,
                    rewards_apy: snapshot.rewards_apy,
                    liquidity_usd: snapshot.liquidity_usd,
                    utilization_rate: snapshot.utilization_rate,
                });
            }
        }

        // Step 4: Forward-fill to create 90 daily data points
        let mut filled_points = Vec::new();
        if !raw_points.is_empty() {
            let mut last_point = raw_points[0].clone();
            let mut raw_idx = 0;

            for day_offset in 0..days_to_fetch {
                let target_date = start + Duration::days(day_offset);
                let target_day_start = Self::get_day_start(target_date);

                // Check if we have a data point for this day
                while raw_idx < raw_points.len() {
                    let point_day_start = Self::get_day_start(raw_points[raw_idx].date);
                    if point_day_start <= target_day_start {
                        last_point = raw_points[raw_idx].clone();
                        if point_day_start == target_day_start {
                            raw_idx += 1;
                            break;
                        }
                        raw_idx += 1;
                    } else {
                        break;
                    }
                }

                // Use last known value (forward-fill)
                filled_points.push(VaultHistoryPoint {
                    date: target_day_start,
                    net_apy: last_point.net_apy,
                    base_apy: last_point.base_apy,
                    rewards_apy: last_point.rewards_apy,
                    liquidity_usd: last_point.liquidity_usd,
                    utilization_rate: last_point.utilization_rate,
                });
            }
        }

        // Step 5: Calculate stats
        let (avg_apy, min_apy, max_apy) = if filled_points.is_empty() {
            tracing::warn!("vault_history: no snapshots found");
            (0.0, 0.0, 0.0)
        } else {
            let apys: Vec<f64> = filled_points.iter().map(|p| p.net_apy).collect();
            let avg = apys.iter().sum::<f64>() / apys.len() as f64;
            let min = apys.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = apys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            tracing::info!(
                "vault_history: returned {} daily points (from {} raw events), avg={:.2}% min={:.2}% max={:.2}%",
                filled_points.len(), raw_points.len(), avg, min, max
            );
            (avg, min, max)
        };

        Ok(crate::models::VaultHistoryResponse {
            success: true,
            vault_id: meta_vault_id.unwrap_or_default(),
            vault_name: meta_vault_name,
            protocol: meta_protocol,
            chain: meta_chain,
            asset: meta_asset,
            operation_type: meta_op_type,
            url: meta_url,
            days: days_to_fetch as u32,
            data_available: !filled_points.is_empty(),
            points: filled_points,
            avg_apy,
            min_apy,
            max_apy,
        })
    }

    // ========== Worker Execution Records Management ==========

    /// Create indexes for execution records collection
    async fn create_execution_indexes(
        collection: &Collection<WorkerExecutionRecord>,
    ) -> Result<()> {
        use mongodb::IndexModel;

        // Index for querying by execution date
        let index_date = IndexModel::builder()
            .keys(doc! { "executedAt": -1 })
            .build();

        // Index for querying by collection date
        let index_collection_date = IndexModel::builder()
            .keys(doc! { "collectionDate": -1 })
            .build();

        // Index for querying by status
        let index_status = IndexModel::builder()
            .keys(doc! { "status": 1, "executedAt": -1 })
            .build();

        collection
            .create_indexes(vec![index_date, index_collection_date, index_status])
            .await?;

        tracing::info!("Created indexes for worker_executions collection");
        Ok(())
    }

    /// Save worker execution record
    pub async fn save_execution_record(&self, record: &WorkerExecutionRecord) -> Result<()> {
        self.execution_records.insert_one(record).await?;

        tracing::info!(
            "Saved execution record: status={:?}, collection_date={}, vaults={}, snapshots_inserted={}, duration={}s",
            record.status,
            record.collection_date,
            record.stats.vaults_processed,
            record.stats.snapshots_inserted,
            record.duration_seconds
        );

        Ok(())
    }

    /// Get latest execution records (last N executions)
    pub async fn get_latest_executions(&self, limit: i64) -> Result<Vec<WorkerExecutionRecord>> {
        let mut cursor = self
            .execution_records
            .find(doc! {})
            .sort(doc! { "executedAt": -1 })
            .limit(limit)
            .await?;

        let mut records = Vec::new();
        while let Some(record) = cursor.next().await {
            records.push(record?);
        }

        Ok(records)
    }

    /// Get executions for a specific date
    pub async fn get_executions_for_date(&self, date: &str) -> Result<Vec<WorkerExecutionRecord>> {
        let mut cursor = self
            .execution_records
            .find(doc! { "collectionDate": date })
            .sort(doc! { "executedAt": -1 })
            .await?;

        let mut records = Vec::new();
        while let Some(record) = cursor.next().await {
            records.push(record?);
        }

        Ok(records)
    }

    /// Count total snapshots in database
    pub async fn count_total_snapshots(&self) -> Result<usize> {
        let count = self.collection.count_documents(doc! {}).await?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Chain, OperationType, Protocol};

    fn make_test_snapshot() -> RateSnapshot {
        RateSnapshot {
            id: None,
            date: Utc::now(),
            vault_id: "test_vault_id".to_string(),
            protocol: Protocol::Aave,
            chain: Chain::Base,
            asset: "USDC".to_string(),
            vault_name: None,
            url: "https://app.aave.com/test".to_string(),
            operation_type: OperationType::Lending,
            action: crate::models::Action::Supply,
            net_apy: 5.0,
            base_apy: 5.0,
            rewards_apy: 0.0,
            liquidity_usd: 1_000_000,
            tvl_usd: 5_000_000,
            utilization_rate: 80,
            metadata: None,
            collected_at: Utc::now(),
        }
    }

    /// CRITICAL: RateSnapshot.date MUST be a BSON DateTime, not a String.
    ///
    /// If serialized as a String ("2026-02-18T00:00:00Z"), MongoDB range queries
    /// using `doc! { "date": { "$gte": start } }` silently return 0 results because
    /// BSON DateTime != BSON String — they are different types in the type system.
    ///
    /// This test would have caught the bug where date-based queries silently returned
    /// 0 results, making the worker re-collect indefinitely without saving data.
    #[test]
    fn test_rate_snapshot_date_is_bson_datetime_not_string() {
        let snapshot = make_test_snapshot();
        let doc =
            bson::to_document(&snapshot).expect("RateSnapshot must serialize to BSON document");

        let date_value = doc
            .get("date")
            .expect("'date' field must exist in serialized BSON");

        assert!(
            matches!(date_value, bson::Bson::DateTime(_)),
            "RateSnapshot.date MUST be BSON DateTime for MongoDB range queries to work. \
             Got {:?} instead. \
             Requires #[serde(with = \"bson::serde_helpers::chrono_datetime_as_bson_datetime\")] \
             on the field.",
            date_value
        );
    }

    /// Same check for collected_at.
    #[test]
    fn test_rate_snapshot_collected_at_is_bson_datetime_not_string() {
        let snapshot = make_test_snapshot();
        let doc = bson::to_document(&snapshot).unwrap();

        let value = doc
            .get("collected_at")
            .expect("'collected_at' field must exist");
        assert!(
            matches!(value, bson::Bson::DateTime(_)),
            "RateSnapshot.collected_at MUST be BSON DateTime. Got {:?}",
            value
        );
    }

    /// get_day_start must truncate time to midnight UTC, preserving the date.
    #[test]
    fn test_get_day_start_truncates_to_midnight() {
        use chrono::{TimeZone, Timelike};
        let dt = Utc.with_ymd_and_hms(2026, 2, 18, 14, 35, 59).unwrap();
        let day_start = HistoricalDataService::get_day_start(dt);

        assert_eq!(day_start.hour(), 0);
        assert_eq!(day_start.minute(), 0);
        assert_eq!(day_start.second(), 0);
        assert_eq!(day_start.date_naive(), dt.date_naive());
    }

    /// get_day_start must be idempotent: applying it twice gives the same result.
    #[test]
    fn test_get_day_start_is_idempotent() {
        use chrono::TimeZone;
        let dt = Utc.with_ymd_and_hms(2026, 2, 18, 10, 0, 0).unwrap();
        let once = HistoricalDataService::get_day_start(dt);
        let twice = HistoricalDataService::get_day_start(once);
        assert_eq!(once, twice, "get_day_start must be idempotent");
    }

    /// Verify that two snapshots for the same vault/date but different actions
    /// produce different vault_ids. This prevents supply vs borrow collisions.
    #[test]
    fn test_vault_id_differs_for_supply_vs_borrow() {
        use crate::models::{Action, RateSnapshot};
        let supply_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Base,
            "USDC",
            "https://app.aave.com/test",
            OperationType::Lending,
            Some(&Action::Supply),
        );
        let borrow_id = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Base,
            "USDC",
            "https://app.aave.com/test",
            OperationType::Lending,
            Some(&Action::Borrow),
        );
        let staking_no_action = RateSnapshot::generate_vault_id(
            &Protocol::Aave,
            &Chain::Base,
            "USDC",
            "https://app.aave.com/test",
            OperationType::Staking,
            None,
        );
        assert_ne!(
            supply_id, borrow_id,
            "Supply and Borrow must have different vault_ids"
        );
        assert_ne!(
            supply_id, staking_no_action,
            "Lending Supply and Staking must have different vault_ids"
        );
        assert_ne!(
            borrow_id, staking_no_action,
            "Lending Borrow and Staking must have different vault_ids"
        );
    }
}
