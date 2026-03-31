use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use mongodb::{Client, Collection, Database};
use mongodb::bson::doc;
use futures::stream::StreamExt;
use std::collections::HashMap;

use crate::models::{PoolSnapshot, PoolResult, PoolHistoryResponse, PoolHistoryPoint};

#[derive(Clone)]
pub struct PoolHistoricalService {
    db: Database,
    collection: Collection<PoolSnapshot>,
}

impl PoolHistoricalService {
    pub async fn new(mongodb_url: &str, database: &str) -> Result<Self> {
        let client = Client::with_uri_str(mongodb_url).await?;
        let db: Database = client.database(database);
        let collection: Collection<PoolSnapshot> = db.collection("pool_snapshots");

        Self::create_indexes(&collection).await?;

        Ok(Self { db, collection })
    }

    async fn create_indexes(collection: &Collection<PoolSnapshot>) -> Result<()> {
        use mongodb::IndexModel;
        use mongodb::options::IndexOptions;

        // Unique constraint: one snapshot per pool per day
        let unique_opts = IndexOptions::builder().unique(true).build();
        let index_unique = IndexModel::builder()
            .keys(doc! { "pool_vault_id": 1, "date": 1 })
            .options(unique_opts)
            .build();

        // Index for protocol/chain queries
        let index_protocol = IndexModel::builder()
            .keys(doc! { "protocol": 1, "chain": 1, "date": -1 })
            .build();

        // Index for normalized pair queries (cross-chain comparison history)
        let index_pair = IndexModel::builder()
            .keys(doc! { "normalized_pair": 1, "date": -1 })
            .build();

        collection.create_indexes(vec![index_unique, index_protocol, index_pair]).await?;

        tracing::info!("MongoDB indexes created for pool_snapshots collection");
        Ok(())
    }

    /// Check if snapshots have already been collected today
    pub async fn has_pool_snapshots_for_today(&self) -> Result<bool> {
        let today = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let today_dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(today, Utc);

        let count = self.collection
            .count_documents(doc! { "date": { "$gte": mongodb::bson::DateTime::from_millis(today_dt.timestamp_millis()) } })
            .await?;

        Ok(count > 0)
    }

    /// Save pool snapshots using event-changed logic (batch version):
    /// 1. Fetches latest snapshot for ALL pools in 1 aggregation (instead of N find_one)
    /// 2. Compares in-memory for event-change
    /// 3. Writes changed snapshots with bounded concurrency
    pub async fn save_pool_snapshots(&self, pools: &[PoolResult], date: DateTime<Utc>) -> Result<usize> {
        if pools.is_empty() {
            return Ok(0);
        }

        let snapshot_date = date
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let snapshot_dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(snapshot_date, Utc);

        // Step 1: Batch fetch latest snapshot per pool (1 aggregation instead of N queries)
        let pool_ids: Vec<&str> = pools.iter().map(|p| p.pool_vault_id.as_str()).collect();
        let pipeline = vec![
            doc! { "$match": { "pool_vault_id": { "$in": &pool_ids } } },
            doc! { "$sort": { "date": -1 } },
            doc! { "$group": {
                "_id": "$pool_vault_id",
                "tvl_usd": { "$first": "$tvl_usd" },
                "volume_24h_usd": { "$first": "$volume_24h_usd" },
                "fee_apr_24h": { "$first": "$fee_apr_24h" },
                "rewards_apr": { "$first": "$rewards_apr" },
            }},
        ];

        let mut cursor = self.db.collection::<mongodb::bson::Document>("pool_snapshots")
            .aggregate(pipeline)
            .await?;

        let mut prev_data: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let Ok(id) = d.get_str("_id") {
                let tvl = d.get_f64("tvl_usd").unwrap_or(0.0);
                let vol = d.get_f64("volume_24h_usd").unwrap_or(0.0);
                let apr = d.get_f64("fee_apr_24h").unwrap_or(0.0);
                let rewards = d.get_f64("rewards_apr").unwrap_or(0.0);
                prev_data.insert(id.to_string(), (tvl, vol, apr, rewards));
            }
        }

        // Step 2: Filter by event-change in memory
        let mut to_save = Vec::new();
        let mut skipped = 0;

        for pool in pools {
            if let Some(&(prev_tvl, prev_vol, prev_apr, prev_rewards)) = prev_data.get(&pool.pool_vault_id) {
                let tvl_changed = (prev_tvl - pool.tvl_usd).abs() > 1.0;
                let volume_changed = (prev_vol - pool.volume_24h_usd).abs() > 1.0;
                let apr_changed = (prev_apr - pool.fee_apr_24h).abs() > 1e-6;
                let rewards_changed = (prev_rewards - pool.rewards_apr).abs() > 1e-6;

                if !tvl_changed && !volume_changed && !apr_changed && !rewards_changed {
                    skipped += 1;
                    continue;
                }
            }
            to_save.push(PoolSnapshot::from_pool_result(pool, snapshot_dt));
        }

        // Step 3: Parallel upsert with bounded concurrency
        let collection = self.collection.clone();
        let upsert_futures: Vec<_> = to_save.into_iter().map(|snapshot| {
            let coll = collection.clone();
            let snap_dt = snapshot_dt;
            async move {
                let filter = doc! {
                    "pool_vault_id": &snapshot.pool_vault_id,
                    "date": mongodb::bson::DateTime::from_millis(snap_dt.timestamp_millis()),
                };
                let update = doc! {
                    "$set": mongodb::bson::to_document(&snapshot).unwrap_or_default()
                };
                coll.update_one(filter, update).upsert(true).await
            }
        }).collect();

        let total = upsert_futures.len();
        let stream = futures::stream::iter(upsert_futures).buffer_unordered(20);
        futures::pin_mut!(stream);

        let mut saved = 0;
        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => saved += 1,
                Err(e) => tracing::warn!("Failed to save pool snapshot: {}", e),
            }
        }

        tracing::info!("Pool snapshots: {} saved, {} skipped (unchanged), {} total", saved, skipped, total + skipped);
        Ok(saved)
    }

    /// Get latest snapshot date for a pool
    pub async fn get_latest_pool_snapshot_date(&self, pool_vault_id: &str) -> Result<Option<DateTime<Utc>>> {
        let latest = self.collection
            .find_one(doc! { "pool_vault_id": pool_vault_id })
            .sort(doc! { "date": -1 })
            .await?;

        Ok(latest.map(|s| s.date))
    }

    /// Get existing snapshot dates for a pool within a date range (for backfill dedup)
    pub async fn get_existing_dates(
        &self,
        pool_vault_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>> {
        let filter = doc! {
            "pool_vault_id": pool_vault_id,
            "date": {
                "$gte": mongodb::bson::DateTime::from_millis(start.timestamp_millis()),
                "$lte": mongodb::bson::DateTime::from_millis(end.timestamp_millis()),
            }
        };

        let mut cursor = self.collection
            .find(filter)
            .sort(doc! { "date": 1 })
            .await?;

        let mut dates = Vec::new();
        while let Some(Ok(snapshot)) = cursor.next().await {
            dates.push(snapshot.date);
        }

        Ok(dates)
    }

    /// Batch check how many snapshot days each pool has within a date range (1 aggregation for N pools).
    /// Returns HashMap<pool_vault_id, count_of_days>.
    pub async fn get_snapshot_counts_batch(
        &self,
        pool_vault_ids: &[&str],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<HashMap<String, usize>> {
        if pool_vault_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let pipeline = vec![
            doc! { "$match": {
                "pool_vault_id": { "$in": pool_vault_ids },
                "date": {
                    "$gte": mongodb::bson::DateTime::from_millis(start.timestamp_millis()),
                    "$lte": mongodb::bson::DateTime::from_millis(end.timestamp_millis()),
                }
            }},
            doc! { "$group": {
                "_id": "$pool_vault_id",
                "count": { "$sum": 1 },
            }},
        ];

        let mut cursor = self.db.collection::<mongodb::bson::Document>("pool_snapshots")
            .aggregate(pipeline)
            .await?;

        let mut counts = HashMap::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let (Ok(id), Ok(count)) = (d.get_str("_id"), d.get_i32("count")) {
                counts.insert(id.to_string(), count as usize);
            }
        }

        Ok(counts)
    }

    /// Batch insert pool snapshots (for backfill). Skips duplicates via ordered=false.
    pub async fn save_pool_snapshots_batch(&self, snapshots: Vec<PoolSnapshot>) -> Result<usize> {
        if snapshots.is_empty() {
            return Ok(0);
        }

        let count = snapshots.len();

        // Use ordered=false so duplicate key errors don't stop the batch
        let opts = mongodb::options::InsertManyOptions::builder()
            .ordered(false)
            .build();

        match self.collection.insert_many(&snapshots).with_options(opts).await {
            Ok(result) => {
                let inserted = result.inserted_ids.len();
                tracing::debug!("Pool backfill: inserted {} of {} snapshots", inserted, count);
                Ok(inserted)
            }
            Err(e) => {
                // Duplicate key errors are expected and OK — count successful inserts
                if e.to_string().contains("E11000") {
                    // Some succeeded, some were duplicates
                    tracing::debug!("Pool backfill: partial insert (some duplicates skipped)");
                    Ok(0) // conservative count
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Get pool history (90 days of daily data points)
    pub async fn get_pool_history(
        &self,
        pool_vault_id: Option<&str>,
        protocol: Option<&crate::models::Protocol>,
        chain: Option<&crate::models::Chain>,
        pair: Option<&str>,
    ) -> Result<PoolHistoryResponse> {
        // Build filter
        let mut filter = doc! {};

        if let Some(vid) = pool_vault_id {
            filter.insert("pool_vault_id", vid);
        } else {
            if let Some(p) = protocol {
                filter.insert("protocol", mongodb::bson::to_bson(p)?);
            }
            if let Some(c) = chain {
                filter.insert("chain", mongodb::bson::to_bson(c)?);
            }
            if let Some(pr) = pair {
                filter.insert("pair", pr);
            }
        }

        // Date range: last 90 days
        let ninety_days_ago = Utc::now() - Duration::days(90);
        filter.insert("date", doc! { "$gte": mongodb::bson::DateTime::from_millis(ninety_days_ago.timestamp_millis()) });

        let mut cursor = self.collection
            .find(filter)
            .sort(doc! { "date": 1 })
            .await?;

        let mut snapshots: Vec<PoolSnapshot> = Vec::new();
        while let Some(result) = cursor.next().await {
            if let Ok(snapshot) = result {
                snapshots.push(snapshot);
            }
        }

        if snapshots.is_empty() {
            let vault_id = pool_vault_id.unwrap_or("unknown").to_string();
            return Ok(PoolHistoryResponse {
                success: true,
                pool_vault_id: vault_id,
                pair: None,
                protocol: None,
                chain: None,
                url: None,
                days: 90,
                points: vec![],
                avg_fee_apr: 0.0,
                min_fee_apr: 0.0,
                max_fee_apr: 0.0,
                avg_tvl: 0.0,
                data_available: false,
            });
        }

        let first = &snapshots[0];
        let vault_id = first.pool_vault_id.clone();

        // Build daily points
        let points: Vec<PoolHistoryPoint> = snapshots.iter().map(|s| {
            let turnover = if s.tvl_usd > 0.0 { s.volume_24h_usd / s.tvl_usd } else { 0.0 };
            PoolHistoryPoint {
                date: s.date,
                tvl_usd: s.tvl_usd,
                volume_24h_usd: s.volume_24h_usd,
                fee_rate_bps: s.fee_rate_bps,
                turnover_ratio_24h: turnover,
                fee_apr_24h: s.fee_apr_24h,
                fee_apr_7d: s.fee_apr_7d,
                rewards_apr: s.rewards_apr,
            }
        }).collect();

        // Summary stats
        let aprs: Vec<f64> = points.iter().map(|p| p.fee_apr_24h).collect();
        let avg_fee_apr = if !aprs.is_empty() { aprs.iter().sum::<f64>() / aprs.len() as f64 } else { 0.0 };
        let min_fee_apr = aprs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_fee_apr = aprs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let avg_tvl = if !points.is_empty() {
            points.iter().map(|p| p.tvl_usd).sum::<f64>() / points.len() as f64
        } else {
            0.0
        };

        Ok(PoolHistoryResponse {
            success: true,
            pool_vault_id: vault_id,
            pair: Some(first.pair.clone()),
            protocol: Some(first.protocol.clone()),
            chain: Some(first.chain.clone()),
            url: Some(first.url.clone()),
            days: 90,
            points,
            avg_fee_apr,
            min_fee_apr: if min_fee_apr.is_infinite() { 0.0 } else { min_fee_apr },
            max_fee_apr: if max_fee_apr.is_infinite() { 0.0 } else { max_fee_apr },
            avg_tvl,
            data_available: true,
        })
    }
}
