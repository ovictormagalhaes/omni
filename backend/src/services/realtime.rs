use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use futures::stream::StreamExt;
use mongodb::bson::{self as bson, doc};
use mongodb::{Client, Collection, Database};

use crate::models::{
    ApyMetrics, Asset, AssetCategory, CurrentRateData, RateSnapshot, RealtimeRate,
};

#[derive(Clone)]
pub struct RealtimeService {
    db: Database,
    collection: Collection<RealtimeRate>,
    snapshots: Collection<RateSnapshot>,
}

impl RealtimeService {
    /// Create new realtime service
    pub async fn new(mongodb_url: &str, database: &str) -> Result<Self> {
        let client = Client::with_uri_str(mongodb_url).await?;
        let db: Database = client.database(database);
        let collection: Collection<RealtimeRate> = db.collection("rate_realtime");
        let snapshots: Collection<RateSnapshot> = db.collection("rate_snapshots");

        // Create indexes
        Self::create_indexes(&collection).await?;

        Ok(Self {
            db,
            collection,
            snapshots,
        })
    }

    /// Create database indexes
    async fn create_indexes(collection: &Collection<RealtimeRate>) -> Result<()> {
        use mongodb::options::IndexOptions;
        use mongodb::IndexModel;

        // Unique constraint on vault_id
        let unique_opts = IndexOptions::builder().unique(true).build();
        let index_vault = IndexModel::builder()
            .keys(doc! { "vault_id": 1 })
            .options(unique_opts)
            .build();

        // Index for protocol/chain queries
        let index_protocol = IndexModel::builder()
            .keys(doc! {
                "protocol": 1,
                "chain": 1,
                "updated_at": -1
            })
            .build();

        // Index for asset queries
        let index_asset = IndexModel::builder()
            .keys(doc! {
                "asset": 1,
                "updated_at": -1
            })
            .build();

        collection
            .create_indexes(vec![index_vault, index_protocol, index_asset])
            .await?;

        tracing::info!("MongoDB indexes created for rate_realtime collection");
        Ok(())
    }

    /// Consolidate a single vault's data into rate_realtime
    pub async fn consolidate_vault(&self, vault_id: &str) -> Result<()> {
        // Get latest snapshot
        let latest_snapshot = self
            .snapshots
            .find_one(doc! { "vault_id": vault_id })
            .sort(doc! { "date": -1 })
            .await?;

        let snapshot = match latest_snapshot {
            Some(s) => s,
            None => {
                tracing::warn!("No snapshots found for vault {}", vault_id);
                return Ok(());
            }
        };

        // Calculate APY metrics from historical data
        let apy_metrics = self.calculate_apy_metrics(vault_id).await?;

        // Count total snapshots
        let snapshot_count = self
            .snapshots
            .count_documents(doc! { "vault_id": vault_id })
            .await? as i32;

        // Get first seen date
        let first_snapshot = self
            .snapshots
            .find_one(doc! { "vault_id": vault_id })
            .sort(doc! { "date": 1 })
            .await?;

        let first_seen = first_snapshot.map(|s| s.date).unwrap_or(snapshot.date);

        // Parse asset to get correct category
        let protocol_str = format!("{:?}", snapshot.protocol);
        let asset = Asset::from_symbol(&snapshot.asset, &protocol_str);
        let asset_category = asset
            .category()
            .first()
            .cloned()
            .unwrap_or(AssetCategory::Other); // Fallback for assets without specific category

        // Build realtime rate document
        let realtime_rate = RealtimeRate {
            id: None,
            vault_id: vault_id.to_string(),
            protocol: snapshot.protocol,
            chain: snapshot.chain,
            asset: snapshot.asset.clone(),
            asset_category,
            vault_name: snapshot.vault_name.clone(),
            url: snapshot.url.clone(),
            operation_type: snapshot.operation_type,
            action: snapshot.action,
            current: CurrentRateData {
                base_apy: snapshot.base_apy,
                rewards_apy: snapshot.rewards_apy,
                net_apy: snapshot.net_apy,
                liquidity_usd: snapshot.liquidity_usd,
                tvl_usd: snapshot.tvl_usd,
                utilization_rate: snapshot.utilization_rate,
                collected_at: snapshot.date,
            },
            apy_metrics,
            updated_at: Utc::now(),
            snapshot_count,
            first_seen,
        };

        // Upsert into rate_realtime
        let filter = doc! { "vault_id": vault_id };
        let mut options = mongodb::options::ReplaceOptions::default();
        options.upsert = Some(true);

        self.collection
            .replace_one(filter, &realtime_rate)
            .with_options(options)
            .await?;

        tracing::debug!(
            "Consolidated vault {} - {}",
            vault_id,
            realtime_rate.protocol
        );

        Ok(())
    }

    /// Calculate APY metrics for a vault
    async fn calculate_apy_metrics(&self, vault_id: &str) -> Result<ApyMetrics> {
        let now = Utc::now();

        // Fetch snapshots for last 90 days
        let start_90d = now - Duration::days(90);

        let filter = doc! {
            "vault_id": vault_id,
            "date": { "$gte": start_90d }
        };

        let mut cursor = self.snapshots.find(filter).sort(doc! { "date": 1 }).await?;

        let mut snapshots = Vec::new();
        while let Some(result) = cursor.next().await {
            if let Ok(snapshot) = result {
                snapshots.push(snapshot);
            }
        }

        if snapshots.is_empty() {
            // No historical data, return zero metrics
            return Ok(ApyMetrics {
                instant: 0.0,
                apy_7d: 0.0,
                apy_30d: 0.0,
                apy_60d: 0.0,
                apy_90d: 0.0,
                volatility: 0.0,
                days_with_data: 0,
            });
        }

        // Get instant APY (latest)
        let instant = snapshots.last().map(|s| s.net_apy).unwrap_or(0.0);

        // Calculate averages
        let apy_7d = self.calculate_average_apy(&snapshots, 7);
        let apy_30d = self.calculate_average_apy(&snapshots, 30);
        let apy_60d = self.calculate_average_apy(&snapshots, 60);
        let apy_90d = self.calculate_average_apy(&snapshots, 90);

        // Calculate volatility (standard deviation)
        let volatility = self.calculate_volatility(&snapshots);

        let days_with_data = snapshots.len() as i32;

        Ok(ApyMetrics {
            instant,
            apy_7d,
            apy_30d,
            apy_60d,
            apy_90d,
            volatility,
            days_with_data,
        })
    }

    /// Calculate average APY for last N days (time-weighted)
    /// This method considers how long each APY value was in effect
    fn calculate_average_apy(&self, snapshots: &[RateSnapshot], days: i64) -> f64 {
        Self::calculate_time_weighted_apy(snapshots, days, Utc::now())
    }

    /// Static helper for time-weighted APY calculation (testable without self)
    fn calculate_time_weighted_apy(
        snapshots: &[RateSnapshot],
        days: i64,
        now: DateTime<Utc>,
    ) -> f64 {
        if snapshots.is_empty() {
            return 0.0;
        }

        let period_start = now - Duration::days(days);

        // Filter snapshots within the period and sort by date
        let mut relevant: Vec<&RateSnapshot> = snapshots
            .iter()
            .filter(|s| s.date >= period_start)
            .collect();

        if relevant.is_empty() {
            // No data in period, use the most recent snapshot before the period
            return snapshots
                .iter()
                .rfind(|s| s.date < period_start)
                .map(|s| s.net_apy)
                .unwrap_or(0.0);
        }

        relevant.sort_by_key(|s| s.date);

        // Calculate time-weighted average
        let mut weighted_sum = 0.0;
        let mut total_duration = 0.0;

        for i in 0..relevant.len() {
            let current = relevant[i];

            // Determine the end date for this APY value
            let end_date = if i + 1 < relevant.len() {
                relevant[i + 1].date
            } else {
                now
            };

            // Calculate duration this APY was in effect (in days)
            let start = if current.date < period_start {
                period_start
            } else {
                current.date
            };

            let duration = (end_date - start).num_seconds() as f64 / 86400.0;

            if duration > 0.0 {
                weighted_sum += current.net_apy * duration;
                total_duration += duration;
            }
        }

        if total_duration > 0.0 {
            weighted_sum / total_duration
        } else {
            relevant.last().map(|s| s.net_apy).unwrap_or(0.0)
        }
    }

    /// Calculate APY volatility (standard deviation)
    fn calculate_volatility(&self, snapshots: &[RateSnapshot]) -> f64 {
        if snapshots.len() < 2 {
            return 0.0;
        }

        let apys: Vec<f64> = snapshots.iter().map(|s| s.net_apy).collect();
        let mean = apys.iter().sum::<f64>() / apys.len() as f64;

        let variance = apys.iter().map(|apy| (apy - mean).powi(2)).sum::<f64>() / apys.len() as f64;

        variance.sqrt()
    }

    /// Consolidate all vaults after collection — pipeline-based (1 aggregation instead of N*5 queries).
    ///
    /// Fetches 90 days of snapshots in a single `$group`, computes time-weighted APY metrics
    /// in Rust, then batch-upserts into `rate_realtime`.
    pub async fn consolidate_all(&self) -> Result<usize> {
        tracing::info!("Starting pipeline-based consolidation of all vaults...");
        let now = Utc::now();
        let start_90d = now - Duration::days(90);

        // --- Step 1: Single aggregation to collect all data per vault ---
        // Groups by vault_id and captures the latest snapshot fields, all APY+date pairs,
        // first_seen, and snapshot count.
        let pipeline = vec![
            doc! { "$sort": { "date": -1 } },
            doc! { "$group": {
                "_id": "$vault_id",
                // Latest snapshot fields (first because sorted desc)
                "latest_date":          { "$first": "$date" },
                "latest_protocol":      { "$first": "$protocol" },
                "latest_chain":         { "$first": "$chain" },
                "latest_asset":         { "$first": "$asset" },
                "latest_vault_name":    { "$first": "$vault_name" },
                "latest_url":           { "$first": "$url" },
                "latest_operation_type": { "$first": "$operation_type" },
                "latest_action":        { "$first": "$action" },
                "latest_base_apy":      { "$first": "$base_apy" },
                "latest_rewards_apy":   { "$first": "$rewards_apy" },
                "latest_net_apy":       { "$first": "$net_apy" },
                "latest_liquidity_usd": { "$first": "$liquidity_usd" },
                "latest_tvl_usd":       { "$first": "$tvl_usd" },
                "latest_utilization_rate": { "$first": "$utilization_rate" },
                // First-seen (oldest)
                "first_seen":           { "$last": "$date" },
                // All APY values + dates for time-weighted calculation
                "all_net_apys":         { "$push": "$net_apy" },
                "all_dates":            { "$push": "$date" },
                // Total count
                "snapshot_count":       { "$sum": 1 },
            }},
        ];

        let mut cursor = self
            .db
            .collection::<bson::Document>("rate_snapshots")
            .aggregate(pipeline)
            .await?;

        let start_90d_millis = start_90d.timestamp_millis();
        let mut consolidated_count = 0u64;
        let mut upsert_futures = Vec::new();

        while let Some(Ok(doc_result)) = cursor.next().await {
            let vault_id = match doc_result.get_str("_id") {
                Ok(id) => id.to_string(),
                Err(_) => continue,
            };

            // --- Extract latest snapshot fields ---
            let latest_base_apy = doc_result.get_f64("latest_base_apy").unwrap_or(0.0);
            let latest_rewards_apy = doc_result.get_f64("latest_rewards_apy").unwrap_or(0.0);
            let latest_net_apy = doc_result.get_f64("latest_net_apy").unwrap_or(0.0);
            let latest_liquidity_usd =
                doc_result.get_i64("latest_liquidity_usd").unwrap_or(0) as u64;
            let latest_tvl_usd = doc_result.get_i64("latest_tvl_usd").unwrap_or(0) as u64;
            let latest_utilization_rate =
                doc_result.get_i32("latest_utilization_rate").unwrap_or(0) as u32;
            let latest_date = doc_result
                .get_datetime("latest_date")
                .ok()
                .map(|d| DateTime::from_timestamp_millis(d.timestamp_millis()).unwrap_or(now))
                .unwrap_or(now);
            let first_seen = doc_result
                .get_datetime("first_seen")
                .ok()
                .map(|d| DateTime::from_timestamp_millis(d.timestamp_millis()).unwrap_or(now))
                .unwrap_or(now);
            let snapshot_count = doc_result.get_i32("snapshot_count").unwrap_or(0);

            // Protocol/chain/asset as strings (stored by serde)
            let protocol_str = doc_result.get_str("latest_protocol").unwrap_or("Aave");
            let chain_str = doc_result.get_str("latest_chain").unwrap_or("Ethereum");
            let asset_str = doc_result.get_str("latest_asset").unwrap_or("USDC");
            let vault_name = doc_result
                .get_str("latest_vault_name")
                .ok()
                .map(|s| s.to_string());
            let url = doc_result.get_str("latest_url").unwrap_or("").to_string();
            let operation_type_str = doc_result
                .get_str("latest_operation_type")
                .unwrap_or("Lending");
            let action_str = doc_result.get_str("latest_action").unwrap_or("Supply");

            let protocol: crate::models::Protocol =
                serde_json::from_value(serde_json::Value::String(protocol_str.to_string()))
                    .unwrap_or(crate::models::Protocol::Aave);
            let chain: crate::models::Chain =
                serde_json::from_value(serde_json::Value::String(chain_str.to_string()))
                    .unwrap_or(crate::models::Chain::Ethereum);
            let operation_type: crate::models::OperationType =
                serde_json::from_value(serde_json::Value::String(operation_type_str.to_string()))
                    .unwrap_or(crate::models::OperationType::Lending);
            let action: crate::models::Action =
                serde_json::from_value(serde_json::Value::String(action_str.to_string()))
                    .unwrap_or(crate::models::Action::Supply);

            // --- Build lightweight snapshots for time-weighted calc (only APY+date needed) ---
            let all_apys: Vec<f64> = doc_result
                .get_array("all_net_apys")
                .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                .unwrap_or_default();
            let all_dates: Vec<bson::DateTime> = doc_result
                .get_array("all_dates")
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_datetime().cloned())
                        .collect()
                })
                .unwrap_or_default();

            // Build RateSnapshot-like structs for the existing time-weighted calculation
            let snapshots_for_calc: Vec<RateSnapshot> = all_apys
                .iter()
                .zip(all_dates.iter())
                .filter(|(_, d)| d.timestamp_millis() >= start_90d_millis)
                .map(|(&apy, d)| {
                    let date = DateTime::from_timestamp_millis(d.timestamp_millis()).unwrap_or(now);
                    RateSnapshot {
                        id: None,
                        vault_id: vault_id.clone(),
                        protocol: protocol.clone(),
                        chain: chain.clone(),
                        asset: asset_str.to_string(),
                        vault_name: None,
                        url: String::new(),
                        operation_type,
                        action: action.clone(),
                        base_apy: 0.0,
                        rewards_apy: 0.0,
                        net_apy: apy,
                        liquidity_usd: 0,
                        tvl_usd: 0,
                        utilization_rate: 0,
                        date,
                        metadata: None,
                        collected_at: date,
                    }
                })
                .collect();

            // Also find the most recent snapshot BEFORE the 90d window for fallback
            let pre_90d_fallback: Option<RateSnapshot> = all_apys
                .iter()
                .zip(all_dates.iter())
                .find(|(_, d)| d.timestamp_millis() < start_90d_millis) // already sorted desc, so first is most recent before window
                .map(|(&apy, d)| {
                    let date = DateTime::from_timestamp_millis(d.timestamp_millis()).unwrap_or(now);
                    RateSnapshot {
                        id: None,
                        vault_id: vault_id.clone(),
                        protocol: protocol.clone(),
                        chain: chain.clone(),
                        asset: asset_str.to_string(),
                        vault_name: None,
                        url: String::new(),
                        operation_type,
                        action: action.clone(),
                        base_apy: 0.0,
                        rewards_apy: 0.0,
                        net_apy: apy,
                        liquidity_usd: 0,
                        tvl_usd: 0,
                        utilization_rate: 0,
                        date,
                        metadata: None,
                        collected_at: date,
                    }
                });

            let mut full_snapshots = Vec::with_capacity(snapshots_for_calc.len() + 1);
            if let Some(fallback) = pre_90d_fallback {
                full_snapshots.push(fallback);
            }
            full_snapshots.extend(snapshots_for_calc);
            // Sort ascending for the calculation (it expects asc order)
            full_snapshots.sort_by_key(|s| s.date);

            let instant = latest_net_apy;
            let apy_7d = Self::calculate_time_weighted_apy(&full_snapshots, 7, now);
            let apy_30d = Self::calculate_time_weighted_apy(&full_snapshots, 30, now);
            let apy_60d = Self::calculate_time_weighted_apy(&full_snapshots, 60, now);
            let apy_90d = Self::calculate_time_weighted_apy(&full_snapshots, 90, now);
            let volatility = Self::calculate_volatility_static(&full_snapshots);
            let days_with_data = full_snapshots.len() as i32;

            let asset_obj = Asset::from_symbol(asset_str, &format!("{:?}", protocol));
            let asset_category = asset_obj
                .category()
                .first()
                .cloned()
                .unwrap_or(AssetCategory::Other);

            let realtime_rate = RealtimeRate {
                id: None,
                vault_id: vault_id.clone(),
                protocol,
                chain,
                asset: asset_str.to_string(),
                asset_category,
                vault_name,
                url,
                operation_type,
                action,
                current: CurrentRateData {
                    base_apy: latest_base_apy,
                    rewards_apy: latest_rewards_apy,
                    net_apy: latest_net_apy,
                    liquidity_usd: latest_liquidity_usd,
                    tvl_usd: latest_tvl_usd,
                    utilization_rate: latest_utilization_rate,
                    collected_at: latest_date,
                },
                apy_metrics: ApyMetrics {
                    instant,
                    apy_7d,
                    apy_30d,
                    apy_60d,
                    apy_90d,
                    volatility,
                    days_with_data,
                },
                updated_at: now,
                snapshot_count,
                first_seen,
            };

            let collection = self.collection.clone();
            upsert_futures.push(async move {
                let filter = doc! { "vault_id": &vault_id };
                let mut options = mongodb::options::ReplaceOptions::default();
                options.upsert = Some(true);
                collection
                    .replace_one(filter, &realtime_rate)
                    .with_options(options)
                    .await
            });
        }

        // Batch upsert with bounded concurrency
        let total = upsert_futures.len();
        tracing::info!(
            "Pipeline aggregation done, upserting {} vaults into rate_realtime...",
            total
        );

        let stream = futures::stream::iter(upsert_futures).buffer_unordered(20);
        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => consolidated_count += 1,
                Err(e) => tracing::warn!("Failed to upsert vault: {:?}", e),
            }
        }

        tracing::info!(
            "Consolidated {} vaults successfully (pipeline)",
            consolidated_count
        );
        Ok(consolidated_count as usize)
    }

    /// Calculate volatility without &self (for pipeline-based consolidation)
    fn calculate_volatility_static(snapshots: &[RateSnapshot]) -> f64 {
        if snapshots.len() < 2 {
            return 0.0;
        }
        let apys: Vec<f64> = snapshots.iter().map(|s| s.net_apy).collect();
        let mean = apys.iter().sum::<f64>() / apys.len() as f64;
        let variance = apys.iter().map(|apy| (apy - mean).powi(2)).sum::<f64>() / apys.len() as f64;
        variance.sqrt()
    }

    /// Get all realtime rates matching query
    pub async fn get_rates(
        &self,
        protocols: Option<&[String]>,
        chains: Option<&[String]>,
        asset: Option<&str>,
    ) -> Result<Vec<RealtimeRate>> {
        let mut filter = doc! {};

        // If protocols specified, use $in operator; otherwise match all
        if let Some(p) = protocols {
            if !p.is_empty() {
                filter.insert("protocol", doc! { "$in": p });
            }
        }

        // If chains specified, use $in operator; otherwise match all
        if let Some(c) = chains {
            if !c.is_empty() {
                filter.insert("chain", doc! { "$in": c });
            }
        }

        if let Some(a) = asset {
            filter.insert("asset", a);
        }

        let mut cursor = self
            .collection
            .find(filter)
            .sort(doc! { "current.net_apy": -1 })
            .await?;

        let mut rates = Vec::new();
        while let Some(result) = cursor.next().await {
            if let Ok(rate) = result {
                rates.push(rate);
            }
        }

        Ok(rates)
    }

    /// Query rates from rate_realtime, applying RateQuery filters.
    /// Returns (results, total_count) for pagination.
    /// Sort: supply → net_apy descending (best first), borrow → net_apy ascending (cheapest first).
    pub async fn query_rates(
        &self,
        query: &crate::models::RateQuery,
    ) -> Result<(Vec<crate::models::RateResult>, u64)> {
        let filter = Self::build_rate_filter(query);

        // Sort: supply descending, borrow ascending
        let sort_doc = match &query.action {
            Some(crate::models::Action::Borrow) => doc! { "current.net_apy": 1 },
            _ => doc! { "current.net_apy": -1 },
        };

        // Count total matching documents
        let total_count = self.collection.count_documents(filter.clone()).await?;

        // Pagination
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let skip = (page - 1) * page_size;

        let mut cursor = self
            .collection
            .find(filter)
            .sort(sort_doc)
            .skip(skip)
            .limit(page_size as i64)
            .await?;

        let mut results = Vec::new();
        while let Some(Ok(rate)) = cursor.next().await {
            let asset = crate::models::Asset::from_symbol(&rate.asset, "realtime");

            results.push(crate::models::RateResult {
                protocol: rate.protocol,
                chain: rate.chain,
                asset: asset.clone(),
                action: rate.action,
                asset_category: asset.category(),
                apy: rate.current.base_apy,
                rewards: rate.current.rewards_apy,
                net_apy: rate.current.net_apy,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                liquidity: rate.current.liquidity_usd,
                total_liquidity: rate.current.tvl_usd,
                utilization_rate: rate.current.utilization_rate,
                operation_type: rate.operation_type,
                url: rate.url,
                vault_id: Some(rate.vault_id),
                vault_name: rate.vault_name,
                last_update: rate.current.collected_at,
                apy_metrics: Some(rate.apy_metrics),
            });
        }

        Ok((results, total_count))
    }

    /// Query ALL matching rates without pagination — for internal use (score, comparisons).
    pub async fn query_all_rates(
        &self,
        query: &crate::models::RateQuery,
    ) -> Result<Vec<crate::models::RateResult>> {
        let filter = Self::build_rate_filter(query);

        let sort_doc = match &query.action {
            Some(crate::models::Action::Borrow) => doc! { "current.net_apy": 1 },
            _ => doc! { "current.net_apy": -1 },
        };

        let mut cursor = self.collection.find(filter).sort(sort_doc).await?;

        let mut results = Vec::new();
        while let Some(Ok(rate)) = cursor.next().await {
            let asset = crate::models::Asset::from_symbol(&rate.asset, "realtime");

            results.push(crate::models::RateResult {
                protocol: rate.protocol,
                chain: rate.chain,
                asset: asset.clone(),
                action: rate.action,
                asset_category: asset.category(),
                apy: rate.current.base_apy,
                rewards: rate.current.rewards_apy,
                net_apy: rate.current.net_apy,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                liquidity: rate.current.liquidity_usd,
                total_liquidity: rate.current.tvl_usd,
                utilization_rate: rate.current.utilization_rate,
                operation_type: rate.operation_type,
                url: rate.url,
                vault_id: Some(rate.vault_id),
                vault_name: rate.vault_name,
                last_update: rate.current.collected_at,
                apy_metrics: Some(rate.apy_metrics),
            });
        }

        Ok(results)
    }

    /// Build MongoDB filter from RateQuery — all filters at DB level for correct pagination.
    fn build_rate_filter(query: &crate::models::RateQuery) -> mongodb::bson::Document {
        let mut filter = doc! {};

        if let Some(protocols) = query.parse_protocols() {
            let p_strings: Vec<String> = protocols
                .iter()
                .map(|p| {
                    serde_json::to_value(p)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            filter.insert("protocol", doc! { "$in": &p_strings });
        }

        if let Some(chains) = query.parse_chains() {
            let c_strings: Vec<String> = chains
                .iter()
                .map(|c| {
                    serde_json::to_value(c)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            filter.insert("chain", doc! { "$in": &c_strings });
        }

        if let Some(ref action) = query.action {
            let a_str = serde_json::to_value(action)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string();
            filter.insert("action", &a_str);
        }

        if query.min_liquidity > 0 {
            filter.insert(
                "current.liquidity_usd",
                doc! { "$gte": query.min_liquidity as i64 },
            );
        }

        // Filter by specific assets
        if let Some(assets) = query.parse_assets() {
            let upper: Vec<String> = assets.iter().map(|a| a.to_uppercase()).collect();
            filter.insert("asset", doc! { "$in": &upper });
        }

        // Filter by asset category — enumerate matching token symbols
        if let Some(categories) = query.parse_asset_categories() {
            let tokens = crate::services::pool_realtime::tokens_for_categories(&categories);
            if !tokens.is_empty() {
                filter.insert("asset", doc! { "$in": &tokens });
            }
        }

        // Filter by token symbol (substring match)
        if let Some(ref token) = query.token {
            let t = regex::escape(token.trim()).to_uppercase();
            if !t.is_empty() {
                filter.insert("asset", doc! { "$regex": &t, "$options": "i" });
            }
        }

        // Filter by operation type
        if let Some(op_types) = query.parse_operation_types() {
            let op_strings: Vec<String> = op_types
                .iter()
                .map(|o| {
                    serde_json::to_value(o)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            filter.insert("operation_type", doc! { "$in": &op_strings });
        }

        filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Chain, OperationType, Protocol, RateSnapshot};
    use chrono::{DateTime, Duration, TimeZone, Utc};

    fn create_test_snapshot(days_ago: i64, apy: f64, base_time: DateTime<Utc>) -> RateSnapshot {
        let date = base_time - Duration::days(days_ago);
        RateSnapshot {
            id: None,
            vault_id: "test_vault".to_string(),
            protocol: Protocol::Aave,
            chain: Chain::Ethereum,
            asset: "USDC".to_string(),
            vault_name: Some("Test Vault".to_string()),
            url: "https://test.com".to_string(),
            operation_type: OperationType::Lending,
            action: crate::models::Action::Supply,
            base_apy: apy,
            rewards_apy: 0.0,
            net_apy: apy,
            liquidity_usd: 1000000,
            tvl_usd: 1000000,
            utilization_rate: 50,
            date,
            metadata: None,
            collected_at: date,
        }
    }

    #[test]
    fn test_calculate_average_apy_single_snapshot() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Single snapshot 15 days ago with 20% APY
        let snapshots = vec![create_test_snapshot(15, 0.20, now)];

        // All periods should return the same value since there's only one data point
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.20).abs() < 0.001,
            "Expected 0.20, got {}",
            avg_30d
        );

        let avg_60d = RealtimeService::calculate_time_weighted_apy(&snapshots, 60, now);
        assert!(
            (avg_60d - 0.20).abs() < 0.001,
            "Expected 0.20, got {}",
            avg_60d
        );

        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!(
            (avg_90d - 0.20).abs() < 0.001,
            "Expected 0.20, got {}",
            avg_90d
        );
    }

    #[test]
    fn test_calculate_average_apy_two_equal_periods() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Day 0 (30 days ago): APY = 10%
        // Day 15 (15 days ago): APY = 20%
        // Day 30 (today): (no snapshot, use last value)
        let snapshots = vec![
            create_test_snapshot(30, 0.10, now),
            create_test_snapshot(15, 0.20, now),
        ];

        // For 30 days: 15 days at 10% + 15 days at 20% = (15*0.10 + 15*0.20) / 30 = 0.15
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.15).abs() < 0.001,
            "Expected 0.15, got {}",
            avg_30d
        );
    }

    #[test]
    fn test_calculate_average_apy_unequal_periods() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Day 0 (30 days ago): APY = 10%
        // Day 25 (5 days ago): APY = 20%
        // Day 30 (today): (no snapshot, use last value)
        let snapshots = vec![
            create_test_snapshot(30, 0.10, now),
            create_test_snapshot(5, 0.20, now),
        ];

        // For 30 days: 25 days at 10% + 5 days at 20% = (25*0.10 + 5*0.20) / 30 = 0.1167
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.1167).abs() < 0.01,
            "Expected ~0.1167, got {}",
            avg_30d
        );
    }

    #[test]
    fn test_calculate_average_apy_recent_change() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Change happened 5 days ago
        // Day 0 (90 days ago): APY = 10%
        // Day 85 (5 days ago): APY = 50%
        let snapshots = vec![
            create_test_snapshot(90, 0.10, now),
            create_test_snapshot(5, 0.50, now),
        ];

        // For 30 days: only the 5-day-ago snapshot is within the period,
        // so we get 5 days at 50% → average = 0.50
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.50).abs() < 0.01,
            "Expected ~0.50, got {}",
            avg_30d
        );

        // For 90 days: 85 days at 10% + 5 days at 50% = (85*0.10 + 5*0.50) / 90 = 0.1222
        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!(
            (avg_90d - 0.1222).abs() < 0.01,
            "Expected ~0.1222, got {}",
            avg_90d
        );
    }

    #[test]
    fn test_calculate_average_apy_no_recent_data() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Only old data (95 days ago)
        let snapshots = vec![create_test_snapshot(95, 0.15, now)];

        // Should return the last known value before the period
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.15).abs() < 0.001,
            "Expected 0.15, got {}",
            avg_30d
        );
    }

    #[test]
    fn test_calculate_average_apy_euler_case() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

        // Simulate the Euler vault scenario: 60% APY
        // If APY changed 7 days ago from 10% to 60%
        let snapshots = vec![
            create_test_snapshot(90, 0.10, now),
            create_test_snapshot(7, 0.60, now),
        ];

        // For 7 days: all 7 days at 60% = 0.60
        let avg_7d = RealtimeService::calculate_time_weighted_apy(&snapshots, 7, now);
        assert!(
            (avg_7d - 0.60).abs() < 0.01,
            "Expected ~0.60, got {}",
            avg_7d
        );

        // For 30 days: only the 7-day-ago snapshot is within the period,
        // so we get 7 days at 60% → average = 0.60
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!(
            (avg_30d - 0.60).abs() < 0.01,
            "Expected ~0.60, got {}",
            avg_30d
        );

        // For 90 days: 83 days at 10% + 7 days at 60% = (83*0.10 + 7*0.60) / 90 = 0.1389
        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!(
            (avg_90d - 0.1389).abs() < 0.01,
            "Expected ~0.1389, got {}",
            avg_90d
        );
    }
}
