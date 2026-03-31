use anyhow::Result;
use chrono::{Duration, Utc};
use futures::stream::StreamExt;
use mongodb::bson::doc;
use mongodb::{Client, Collection, Database};

use crate::models::{
    Asset, AssetCategory, CurrentPoolData, KnownAsset, PoolAprMetrics, PoolQuery, PoolResult,
    PoolSnapshot, RealtimePool,
};
/// Return all token symbols that belong to any of the given asset categories.
pub fn tokens_for_categories(categories: &[AssetCategory]) -> Vec<String> {
    use KnownAsset::*;
    let all_known = vec![
        USDC, USDT, DAI, USDE, SUSDE, PYUSD, FRAX, LUSD, GHO, CRVUSD, USDD, EURC, EURS, EURT, ETH,
        WETH, STETH, WSTETH, RETH, CBETH, SETH2, SFRXETH, WBTC, CBBTC, TBTC, SBTC, SOL, STSOL,
        MSOL, JITOSOL, JUPSOL, TRX, LINK, AAVE, UNI, CRV, BAL, COMP,
    ];
    let mut tokens = Vec::new();
    for known in all_known {
        let asset_cats = known.category();
        if asset_cats.iter().any(|c| categories.contains(c)) {
            tokens.push(format!("{:?}", known).to_uppercase());
        }
    }
    tokens
}

#[derive(Clone)]
pub struct PoolRealtimeService {
    db: Database,
    collection: Collection<RealtimePool>,
    #[allow(dead_code)]
    snapshots: Collection<PoolSnapshot>,
}

impl PoolRealtimeService {
    pub async fn new(mongodb_url: &str, database: &str) -> Result<Self> {
        let client = Client::with_uri_str(mongodb_url).await?;
        let db: Database = client.database(database);
        let collection: Collection<RealtimePool> = db.collection("pool_realtime");
        let snapshots: Collection<PoolSnapshot> = db.collection("pool_snapshots");

        Self::create_indexes(&collection).await?;

        Ok(Self {
            db,
            collection,
            snapshots,
        })
    }

    async fn create_indexes(collection: &Collection<RealtimePool>) -> Result<()> {
        use mongodb::options::IndexOptions;
        use mongodb::IndexModel;

        let unique_opts = IndexOptions::builder().unique(true).build();
        let index_vault = IndexModel::builder()
            .keys(doc! { "pool_vault_id": 1 })
            .options(unique_opts)
            .build();

        let index_pair = IndexModel::builder()
            .keys(doc! { "normalized_pair": 1, "updated_at": -1 })
            .build();

        let index_protocol = IndexModel::builder()
            .keys(doc! { "protocol": 1, "chain": 1, "updated_at": -1 })
            .build();

        collection
            .create_indexes(vec![index_vault, index_pair, index_protocol])
            .await?;

        tracing::info!("MongoDB indexes created for pool_realtime collection");
        Ok(())
    }

    // ========================================================================
    // WRITE: Called by the worker to save fresh pool data
    // ========================================================================

    /// Upsert pool_realtime directly from PoolResult (fresh indexer data).
    /// Uses bounded concurrency instead of sequential loop.
    pub async fn upsert_from_results(&self, pools: &[PoolResult]) -> Result<usize> {
        if pools.is_empty() {
            return Ok(0);
        }

        let now = Utc::now();
        let collection = self.collection.clone();

        let upsert_futures: Vec<_> = pools.iter().map(|pool| {
            let coll = collection.clone();
            let pool = pool.clone();
            async move {
                let realtime = RealtimePool {
                    id: None,
                    pool_vault_id: pool.pool_vault_id.clone(),
                    protocol: pool.protocol.clone(),
                    chain: pool.chain.clone(),
                    token0: pool.token0.clone(),
                    token1: pool.token1.clone(),
                    pair: pool.pair.clone(),
                    normalized_pair: pool.normalized_pair.clone(),
                    pool_type: pool.pool_type.clone(),
                    fee_rate_bps: pool.fee_rate_bps,
                    url: pool.url.clone(),
                    current: CurrentPoolData {
                        tvl_usd: pool.tvl_usd,
                        volume_24h_usd: pool.volume_24h_usd,
                        volume_7d_usd: pool.volume_7d_usd,
                        fees_24h_usd: pool.fees_24h_usd,
                        fees_7d_usd: pool.fees_7d_usd,
                        turnover_ratio_24h: pool.turnover_ratio_24h,
                        turnover_ratio_7d: pool.turnover_ratio_7d,
                        fee_apr_24h: pool.fee_apr_24h,
                        fee_apr_7d: pool.fee_apr_7d,
                        rewards_apr: pool.rewards_apr,
                        collected_at: now,
                    },
                    fee_apr_metrics: PoolAprMetrics {
                        instant: pool.fee_apr_24h,
                        apr_7d: pool.fee_apr_7d,
                        apr_30d: pool.fee_apr_24h,
                        volatility: 0.0,
                        days_with_data: 1,
                    },
                    updated_at: now,
                    snapshot_count: 0,
                    first_seen: now,
                };

                let filter = doc! { "pool_vault_id": &pool.pool_vault_id };
                let mut update_doc = mongodb::bson::to_document(&realtime)?;
                update_doc.remove("first_seen");
                update_doc.remove("_id");
                let update = doc! {
                    "$set": update_doc,
                    "$setOnInsert": { "first_seen": mongodb::bson::DateTime::from_millis(now.timestamp_millis()) }
                };

                coll.update_one(filter, update).upsert(true).await
                    .map(|_| ())
                    .map_err(|e| anyhow::anyhow!("Failed to upsert pool {}: {}", pool.pool_vault_id, e))
            }
        }).collect();

        let total = upsert_futures.len();
        let stream = futures::stream::iter(upsert_futures).buffer_unordered(20);
        futures::pin_mut!(stream);

        let mut saved = 0;
        while let Some(result) = stream.next().await {
            match result {
                Ok(()) => saved += 1,
                Err(e) => tracing::warn!("{}", e),
            }
        }

        tracing::info!("Upserted {}/{} pools into pool_realtime", saved, total);
        Ok(saved)
    }

    // ========================================================================
    // READ: Called by the API route to serve pool data from mongo
    // ========================================================================

    /// Query pools from pool_realtime collection, applying filters.
    /// This is what the GET /api/v1/pools endpoint calls.
    /// Returns (results, total_count) for pagination.
    pub async fn query_pools(&self, query: &PoolQuery) -> Result<(Vec<PoolResult>, u64)> {
        let filter = Self::build_pool_filter(query);

        // Always sort by Fee APR descending (best first)
        let sort_doc = doc! { "current.fee_apr_24h": -1 };

        // Count total matching documents (for pagination metadata)
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

        let mut results: Vec<PoolResult> = Vec::new();
        while let Some(Ok(p)) = cursor.next().await {
            let token0_asset = Asset::from_symbol(&p.token0, "pool");
            let token1_asset = Asset::from_symbol(&p.token1, "pool");

            results.push(PoolResult {
                protocol: p.protocol,
                chain: p.chain,
                token0: p.token0.clone(),
                token1: p.token1.clone(),
                pair: p.pair,
                normalized_pair: p.normalized_pair,
                token0_categories: token0_asset.category(),
                token1_categories: token1_asset.category(),
                pool_type: p.pool_type,
                fee_tier: crate::models::FeeTier::from_bps(p.fee_rate_bps).display(),
                fee_rate_bps: p.fee_rate_bps,
                tvl_usd: p.current.tvl_usd,
                volume_24h_usd: p.current.volume_24h_usd,
                volume_7d_usd: p.current.volume_7d_usd,
                turnover_ratio_24h: p.current.turnover_ratio_24h,
                turnover_ratio_7d: p.current.turnover_ratio_7d,
                fees_24h_usd: p.current.fees_24h_usd,
                fees_7d_usd: p.current.fees_7d_usd,
                fee_apr_24h: p.current.fee_apr_24h,
                fee_apr_7d: p.current.fee_apr_7d,
                rewards_apr: p.current.rewards_apr,
                total_apr: p.current.fee_apr_24h + p.current.rewards_apr,
                pool_address: String::new(),
                url: p.url,
                last_update: p.current.collected_at,
                pool_vault_id: p.pool_vault_id,
            });
        }

        Ok((results, total_count))
    }

    /// Query ALL matching pools without pagination — for internal use (score, comparisons).
    pub async fn query_all_pools(&self, query: &PoolQuery) -> Result<Vec<PoolResult>> {
        let filter = Self::build_pool_filter(query);
        let sort_doc = doc! { "current.fee_apr_24h": -1 };

        let mut cursor = self.collection.find(filter).sort(sort_doc).await?;

        let mut results: Vec<PoolResult> = Vec::new();
        while let Some(Ok(p)) = cursor.next().await {
            let token0_asset = Asset::from_symbol(&p.token0, "pool");
            let token1_asset = Asset::from_symbol(&p.token1, "pool");

            results.push(PoolResult {
                protocol: p.protocol,
                chain: p.chain,
                token0: p.token0.clone(),
                token1: p.token1.clone(),
                pair: p.pair,
                normalized_pair: p.normalized_pair,
                token0_categories: token0_asset.category(),
                token1_categories: token1_asset.category(),
                pool_type: p.pool_type,
                fee_tier: crate::models::FeeTier::from_bps(p.fee_rate_bps).display(),
                fee_rate_bps: p.fee_rate_bps,
                tvl_usd: p.current.tvl_usd,
                volume_24h_usd: p.current.volume_24h_usd,
                volume_7d_usd: p.current.volume_7d_usd,
                turnover_ratio_24h: p.current.turnover_ratio_24h,
                turnover_ratio_7d: p.current.turnover_ratio_7d,
                fees_24h_usd: p.current.fees_24h_usd,
                fees_7d_usd: p.current.fees_7d_usd,
                fee_apr_24h: p.current.fee_apr_24h,
                fee_apr_7d: p.current.fee_apr_7d,
                rewards_apr: p.current.rewards_apr,
                total_apr: p.current.fee_apr_24h + p.current.rewards_apr,
                pool_address: String::new(),
                url: p.url,
                last_update: p.current.collected_at,
                pool_vault_id: p.pool_vault_id,
            });
        }

        Ok(results)
    }

    /// Build MongoDB filter from PoolQuery — all filters at DB level for correct pagination.
    fn build_pool_filter(query: &PoolQuery) -> mongodb::bson::Document {
        let mut filter = doc! {};

        // Filter by protocol
        if let Some(protocols) = query.parse_protocols() {
            let protocol_strings: Vec<String> = protocols
                .iter()
                .map(|p| {
                    serde_json::to_value(p)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            filter.insert("protocol", doc! { "$in": &protocol_strings });
        }

        // Filter by chain
        if let Some(chains) = query.parse_chains() {
            let chain_strings: Vec<String> = chains
                .iter()
                .map(|c| {
                    serde_json::to_value(c)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            filter.insert("chain", doc! { "$in": &chain_strings });
        }

        // Filter by pool type
        if let Some(ref pt) = query.parse_pool_type() {
            let pt_str = serde_json::to_value(pt)
                .unwrap_or_default()
                .as_str()
                .unwrap_or("")
                .to_string();
            filter.insert("pool_type", &pt_str);
        }

        // Filter by min TVL
        filter.insert("current.tvl_usd", doc! { "$gte": query.min_tvl as f64 });

        // Filter by min volume
        if query.min_volume > 0 {
            filter.insert(
                "current.volume_24h_usd",
                doc! { "$gte": query.min_volume as f64 },
            );
        }

        // Filter by token_a (substring match on token0)
        if let Some(ref token_a) = query.token_a {
            let t = regex::escape(token_a.trim()).to_uppercase();
            if !t.is_empty() {
                filter.insert("token0", doc! { "$regex": &t, "$options": "i" });
            }
        }

        // Filter by token_b (substring match on token1)
        if let Some(ref token_b) = query.token_b {
            let t = regex::escape(token_b.trim()).to_uppercase();
            if !t.is_empty() {
                filter.insert("token1", doc! { "$regex": &t, "$options": "i" });
            }
        }

        // Legacy: filter by token (substring match on either side)
        if let Some(ref token) = query.token {
            let t = regex::escape(token.trim()).to_uppercase();
            if !t.is_empty() {
                filter.insert(
                    "$or",
                    vec![
                        doc! { "token0": { "$regex": &t, "$options": "i" } },
                        doc! { "token1": { "$regex": &t, "$options": "i" } },
                    ],
                );
            }
        }

        // Filter by exact pair (either order)
        if let Some(ref pair) = query.pair {
            let p = pair.trim().to_uppercase();
            if let Some((a, b)) = p.split_once('/') {
                let reversed = format!("{}/{}", b, a);
                filter.insert("$or", vec![doc! { "pair": &p }, doc! { "pair": &reversed }]);
            }
        }

        // Filter by normalized pair
        if let Some(ref np) = query.normalized_pair {
            filter.insert("normalized_pair", np.as_str());
        }

        // Filter by asset categories (two-sided: token side 0 + token side 1)
        let cats0 = query.parse_asset_categories_0();
        let cats1 = query.parse_asset_categories_1();
        let tokens0 = cats0
            .as_ref()
            .map(|c| tokens_for_categories(c))
            .unwrap_or_default();
        let tokens1 = cats1
            .as_ref()
            .map(|c| tokens_for_categories(c))
            .unwrap_or_default();

        let cat_filter = if !tokens0.is_empty() && !tokens1.is_empty() {
            // Both sides specified: match pools where one token matches side0 and the other side1 (either order)
            Some(doc! {
                "$or": [
                    { "$and": [ { "token0": { "$in": &tokens0 } }, { "token1": { "$in": &tokens1 } } ] },
                    { "$and": [ { "token0": { "$in": &tokens1 } }, { "token1": { "$in": &tokens0 } } ] },
                ]
            })
        } else if !tokens0.is_empty() {
            // Only side 0: match either token
            Some(doc! {
                "$or": [
                    { "token0": { "$in": &tokens0 } },
                    { "token1": { "$in": &tokens0 } },
                ]
            })
        } else if !tokens1.is_empty() {
            // Only side 1: match either token
            Some(doc! {
                "$or": [
                    { "token0": { "$in": &tokens1 } },
                    { "token1": { "$in": &tokens1 } },
                ]
            })
        } else {
            None
        };

        if let Some(cat_filter) = cat_filter {
            if filter.contains_key("$or") {
                let existing_or = filter.remove("$or").unwrap();
                filter.insert("$and", vec![doc! { "$or": existing_or }, cat_filter]);
            } else {
                // Merge cat_filter keys into the main filter
                for (key, val) in cat_filter {
                    filter.insert(key, val);
                }
            }
        }

        filter
    }

    // ========================================================================
    // CONSOLIDATE: Enriches pool_realtime with historical metrics from snapshots
    // ========================================================================

    /// P2: Consolidate ALL pools in a single aggregation pipeline instead of N*4 queries.
    /// Computes 7d/30d avg APR, volatility, and snapshot count for each pool.
    pub async fn consolidate_all(&self) -> Result<usize> {
        let now = Utc::now();
        let thirty_days_ago = now - Duration::days(30);
        let seven_days_ago = now - Duration::days(7);

        // Single aggregation: group by pool_vault_id, compute metrics
        let pipeline = vec![
            doc! { "$match": {
                "date": { "$gte": mongodb::bson::DateTime::from_millis(thirty_days_ago.timestamp_millis()) }
            }},
            doc! { "$sort": { "date": -1 } },
            doc! { "$group": {
                "_id": "$pool_vault_id",
                "latest_apr": { "$first": "$fee_apr_24h" },
                "all_aprs": { "$push": "$fee_apr_24h" },
                "all_dates": { "$push": "$date" },
                "count": { "$sum": 1 },
            }},
        ];

        let db = &self.db;
        let mut cursor = db
            .collection::<mongodb::bson::Document>("pool_snapshots")
            .aggregate(pipeline)
            .await?;

        // Also get total snapshot counts per pool (including older than 30d)
        let count_pipeline =
            vec![doc! { "$group": { "_id": "$pool_vault_id", "total": { "$sum": 1 } } }];
        let mut count_cursor = db
            .collection::<mongodb::bson::Document>("pool_snapshots")
            .aggregate(count_pipeline)
            .await?;

        let mut total_counts: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        while let Some(Ok(doc)) = count_cursor.next().await {
            if let (Some(id), Some(total)) = (doc.get_str("_id").ok(), doc.get_i32("total").ok()) {
                total_counts.insert(id.to_string(), total);
            }
        }

        let seven_days_ago_bson =
            mongodb::bson::DateTime::from_millis(seven_days_ago.timestamp_millis());

        // Collect all update operations from the aggregation cursor
        let mut updates: Vec<(String, mongodb::bson::Document)> = Vec::new();

        while let Some(Ok(agg_doc)) = cursor.next().await {
            let pool_id = match agg_doc.get_str("_id") {
                Ok(id) => id.to_string(),
                Err(_) => continue,
            };

            let latest_apr = agg_doc.get_f64("latest_apr").unwrap_or(0.0);
            let all_aprs: Vec<f64> = agg_doc
                .get_array("all_aprs")
                .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                .unwrap_or_default();
            let all_dates: Vec<mongodb::bson::DateTime> = agg_doc
                .get_array("all_dates")
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_datetime().cloned())
                        .collect()
                })
                .unwrap_or_default();

            let apr_7d_values: Vec<f64> = all_aprs
                .iter()
                .zip(all_dates.iter())
                .filter(|(_, d)| **d >= seven_days_ago_bson)
                .map(|(apr, _)| *apr)
                .collect();

            let apr_7d = if !apr_7d_values.is_empty() {
                apr_7d_values.iter().sum::<f64>() / apr_7d_values.len() as f64
            } else {
                latest_apr
            };

            let apr_30d = if !all_aprs.is_empty() {
                all_aprs.iter().sum::<f64>() / all_aprs.len() as f64
            } else {
                latest_apr
            };

            let volatility = if all_aprs.len() > 1 {
                let mean = apr_30d;
                let variance = all_aprs.iter().map(|apr| (apr - mean).powi(2)).sum::<f64>()
                    / (all_aprs.len() - 1) as f64;
                variance.sqrt()
            } else {
                0.0
            };

            let snapshot_count = total_counts
                .get(&pool_id)
                .copied()
                .unwrap_or(all_aprs.len() as i32);

            if let Ok(metrics_bson) = mongodb::bson::to_bson(&PoolAprMetrics {
                instant: latest_apr,
                apr_7d,
                apr_30d,
                volatility,
                days_with_data: all_aprs.len() as i32,
            }) {
                let update = doc! {
                    "$set": {
                        "fee_apr_metrics": metrics_bson,
                        "snapshot_count": snapshot_count,
                    }
                };
                updates.push((pool_id, update));
            }
        }

        // Parallel upsert with bounded concurrency
        let total = updates.len();
        tracing::info!(
            "Pool consolidation: upserting {} pools with metrics...",
            total
        );

        let collection = self.collection.clone();
        let update_futures: Vec<_> = updates
            .into_iter()
            .map(|(pool_id, update)| {
                let coll = collection.clone();
                async move {
                    coll.update_one(doc! { "pool_vault_id": &pool_id }, update)
                        .await
                }
            })
            .collect();

        let stream = futures::stream::iter(update_futures).buffer_unordered(20);
        futures::pin_mut!(stream);

        let mut consolidated = 0u64;
        while let Some(result) = stream.next().await {
            if result.is_ok() {
                consolidated += 1;
            }
        }

        tracing::info!(
            "Consolidated {} pools into pool_realtime (aggregation pipeline)",
            consolidated
        );
        Ok(consolidated as usize)
    }
}
