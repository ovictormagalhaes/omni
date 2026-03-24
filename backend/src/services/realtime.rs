use anyhow::Result;
use chrono::{Utc, Duration, DateTime};
use mongodb::{Client, Collection, Database};
use mongodb::bson::doc;
use futures::stream::StreamExt;

use crate::models::{RealtimeRate, RateSnapshot, CurrentRateData, ApyMetrics, AssetCategory, Asset};

#[derive(Clone)]
pub struct RealtimeService {
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
        
        Ok(Self { collection, snapshots })
    }
    
    /// Create database indexes
    async fn create_indexes(collection: &Collection<RealtimeRate>) -> Result<()> {
        use mongodb::IndexModel;
        use mongodb::options::IndexOptions;
        
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
        
        collection.create_indexes(vec![index_vault, index_protocol, index_asset]).await?;
        
        tracing::info!("MongoDB indexes created for rate_realtime collection");
        Ok(())
    }
    
    /// Consolidate a single vault's data into rate_realtime
    pub async fn consolidate_vault(&self, vault_id: &str) -> Result<()> {
        // Get latest snapshot
        let latest_snapshot = self.snapshots
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
        let snapshot_count = self.snapshots
            .count_documents(doc! { "vault_id": vault_id })
            .await? as i32;
        
        // Get first seen date
        let first_snapshot = self.snapshots
            .find_one(doc! { "vault_id": vault_id })
            .sort(doc! { "date": 1 })
            .await?;
        
        let first_seen = first_snapshot
            .map(|s| s.date)
            .unwrap_or(snapshot.date);
        
        // Parse asset to get correct category
        let protocol_str = format!("{:?}", snapshot.protocol);
        let asset = Asset::from_symbol(&snapshot.asset, &protocol_str);
        let asset_category = asset.category()
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
        
        tracing::debug!("Consolidated vault {} - {}", vault_id, realtime_rate.protocol);
        
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
        
        let mut cursor = self.snapshots
            .find(filter)
            .sort(doc! { "date": 1 })
            .await?;
        
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
    fn calculate_time_weighted_apy(snapshots: &[RateSnapshot], days: i64, now: DateTime<Utc>) -> f64 {
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
                .filter(|s| s.date < period_start)
                .last()
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
        
        let variance = apys.iter()
            .map(|apy| (apy - mean).powi(2))
            .sum::<f64>() / apys.len() as f64;
        
        variance.sqrt()
    }
    
    /// Consolidate all vaults after collection
    pub async fn consolidate_all(&self) -> Result<usize> {
        tracing::info!("Starting consolidation of all vaults...");
        
        // Get all distinct vault_ids from snapshots
        let pipeline = vec![
            doc! { "$group": { "_id": "$vault_id" } },
            doc! { "$project": { "_id": 1 } },
        ];
        
        let mut cursor = self.snapshots.aggregate(pipeline).await?;
        let mut vault_ids = Vec::new();
        
        while let Some(result) = cursor.next().await {
            if let Ok(doc) = result {
                if let Ok(vault_id) = doc.get_str("_id") {
                    vault_ids.push(vault_id.to_string());
                }
            }
        }
        
        tracing::info!("Found {} unique vaults to consolidate", vault_ids.len());
        
        let mut consolidated_count = 0;
        for vault_id in &vault_ids {
            if let Err(e) = self.consolidate_vault(vault_id).await {
                tracing::warn!("Failed to consolidate vault {}: {:?}", vault_id, e);
            } else {
                consolidated_count += 1;
            }
        }
        
        tracing::info!("Consolidated {} vaults successfully", consolidated_count);
        
        Ok(consolidated_count)
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
        
        let mut cursor = self.collection
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RateSnapshot, Protocol, Chain, OperationType};
    use chrono::{Utc, Duration, TimeZone, DateTime};

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
        assert!((avg_30d - 0.20).abs() < 0.001, "Expected 0.20, got {}", avg_30d);
        
        let avg_60d = RealtimeService::calculate_time_weighted_apy(&snapshots, 60, now);
        assert!((avg_60d - 0.20).abs() < 0.001, "Expected 0.20, got {}", avg_60d);
        
        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!((avg_90d - 0.20).abs() < 0.001, "Expected 0.20, got {}", avg_90d);
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
        assert!((avg_30d - 0.15).abs() < 0.001, "Expected 0.15, got {}", avg_30d);
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
        assert!((avg_30d - 0.1167).abs() < 0.01, "Expected ~0.1167, got {}", avg_30d);
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
        
        // For 30 days: 25 days at 10% + 5 days at 50% = (25*0.10 + 5*0.50) / 30 = 0.1667
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!((avg_30d - 0.1667).abs() < 0.01, "Expected ~0.1667, got {}", avg_30d);
        
        // For 90 days: 85 days at 10% + 5 days at 50% = (85*0.10 + 5*0.50) / 90 = 0.1222
        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!((avg_90d - 0.1222).abs() < 0.01, "Expected ~0.1222, got {}", avg_90d);
    }

    #[test]
    fn test_calculate_average_apy_no_recent_data() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        
        // Only old data (95 days ago)
        let snapshots = vec![create_test_snapshot(95, 0.15, now)];
        
        // Should return the last known value before the period
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!((avg_30d - 0.15).abs() < 0.001, "Expected 0.15, got {}", avg_30d);
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
        assert!((avg_7d - 0.60).abs() < 0.01, "Expected ~0.60, got {}", avg_7d);
        
        // For 30 days: 23 days at 10% + 7 days at 60% = (23*0.10 + 7*0.60) / 30 = 0.2167
        let avg_30d = RealtimeService::calculate_time_weighted_apy(&snapshots, 30, now);
        assert!((avg_30d - 0.2167).abs() < 0.01, "Expected ~0.2167, got {}", avg_30d);
        
        // For 90 days: 83 days at 10% + 7 days at 60% = (83*0.10 + 7*0.60) / 90 = 0.1389
        let avg_90d = RealtimeService::calculate_time_weighted_apy(&snapshots, 90, now);
        assert!((avg_90d - 0.1389).abs() < 0.01, "Expected ~0.1389, got {}", avg_90d);
    }
}
