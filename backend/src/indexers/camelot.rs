use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

// ============================================================================
// Camelot V3 — The Graph Subgraph (Algebra-based CL with dynamic fees)
// ============================================================================
// Direct data from on-chain via The Graph decentralized network.
// Camelot V3 uses Algebra protocol (not standard Uniswap V3).
// Key difference: dynamic fees (`fee` field) instead of fixed `feeTier`.
// Schema: pools with poolDayData, similar to Uni V3 but fee is per-pool dynamic.
// Supported chains: Arbitrum
// ============================================================================

const SUBGRAPH_ID: &str = "7mPnp1UqmefcCycB8umy4uUkTkFxMoHn1Y7ncBUscePp";

#[derive(Clone)]
pub struct CamelotIndexer {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    pools: Vec<SubgraphPool>,
}

#[derive(Debug, Deserialize)]
struct SubgraphPool {
    id: String,
    token0: TokenInfo,
    token1: TokenInfo,
    // Algebra uses dynamic `fee` (in hundredths of a bip, i.e. 1e-6) instead of fixed feeTier
    fee: Option<String>,
    #[serde(rename = "feeTier")]
    fee_tier: Option<String>,
    #[serde(rename = "totalValueLockedUSD")]
    total_value_locked_usd: String,
    #[allow(dead_code)]
    #[serde(rename = "volumeUSD")]
    volume_usd: String,
    #[serde(rename = "poolDayData")]
    pool_day_data: Vec<PoolDayData>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    symbol: String,
    #[allow(dead_code)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct PoolDayData {
    #[allow(dead_code)]
    date: i64,
    #[serde(rename = "volumeUSD")]
    volume_usd: String,
    #[serde(rename = "tvlUSD")]
    tvl_usd: String,
    #[serde(rename = "feesUSD")]
    fees_usd: String,
}

impl CamelotIndexer {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            graph_api_key,
        }
    }

    pub async fn fetch_pools_for_chain(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        if *chain != Chain::Arbitrum {
            return Ok(vec![]);
        }

        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[Camelot] No THE_GRAPH_API_KEY configured, skipping");
                return Ok(vec![]);
            }
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, SUBGRAPH_ID
        );

        // Algebra schema: `fee` instead of `feeTier`, but otherwise similar to Uni V3
        let query = serde_json::json!({
            "query": r#"
            {
                pools(
                    first: 200,
                    orderBy: totalValueLockedUSD,
                    orderDirection: desc,
                    where: { totalValueLockedUSD_gt: "10000" }
                ) {
                    id
                    token0 { symbol, id }
                    token1 { symbol, id }
                    fee
                    feeTier
                    totalValueLockedUSD
                    volumeUSD
                    poolDayData(first: 7, orderBy: date, orderDirection: desc) {
                        date
                        volumeUSD
                        tvlUSD
                        feesUSD
                    }
                }
            }
            "#
        });

        tracing::info!("[Camelot] Fetching pools from Algebra subgraph on Arbitrum");

        let http_response = self.client
            .post(&url)
            .json(&query)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!("[Camelot] HTTP {}: {}", status, &body[..body.len().min(200)]);
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Camelot] Failed to parse response: {} — body: {}", e, &body[..body.len().min(200)]);
                return Ok(vec![]);
            }
        };

        let pools = match response.data {
            Some(data) => data.pools,
            None => {
                tracing::warn!("[Camelot] No data returned");
                return Ok(vec![]);
            }
        };

        tracing::info!("[Camelot] Fetched {} pools from subgraph", pools.len());

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(Self::parse_pool)
            .collect();

        tracing::info!("[Camelot] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(pool: SubgraphPool) -> Option<PoolRate> {
        let tvl: f64 = pool.total_value_locked_usd.parse().unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        // Algebra dynamic fee: stored as hundredths of a bip (1e-6 units)
        // e.g. fee=3000 means 0.30%, fee=500 means 0.05%
        // Same encoding as Uniswap V3 feeTier
        let fee_raw: u32 = pool.fee.as_deref()
            .or(pool.fee_tier.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);
        let fee_tier = FeeTier::from_uniswap_fee(fee_raw);
        let fee_rate_bps = fee_tier.to_bps();

        let token0 = Asset::from_symbol(&pool.token0.symbol, "Camelot");
        let token1 = Asset::from_symbol(&pool.token1.symbol, "Camelot");

        // 24h metrics
        let (fees_24h, volume_24h, fee_apr_24h) = if let Some(day) = pool.pool_day_data.first() {
            let fees: f64 = day.fees_usd.parse().unwrap_or(0.0);
            let vol: f64 = day.volume_usd.parse().unwrap_or(0.0);
            let day_tvl: f64 = day.tvl_usd.parse().unwrap_or(tvl);
            let apr = if day_tvl > 0.0 { (fees * 365.0 / day_tvl) * 100.0 } else { 0.0 };
            (fees, vol, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        // 7-day averages — extrapolate to 7 days when fewer days of data are available
        let (fees_7d, volume_7d, fee_apr_7d) = if !pool.pool_day_data.is_empty() {
            let days = pool.pool_day_data.len() as f64;
            let total_fees: f64 = pool.pool_day_data.iter()
                .map(|d| d.fees_usd.parse::<f64>().unwrap_or(0.0))
                .sum();
            let total_volume: f64 = pool.pool_day_data.iter()
                .map(|d| d.volume_usd.parse::<f64>().unwrap_or(0.0))
                .sum();
            let avg_tvl: f64 = pool.pool_day_data.iter()
                .map(|d| d.tvl_usd.parse::<f64>().unwrap_or(0.0))
                .sum::<f64>() / days;
            let daily_avg_fees = total_fees / days;
            let daily_avg_volume = total_volume / days;
            let fees_7d = daily_avg_fees * 7.0;
            let volume_7d = daily_avg_volume * 7.0;
            let apr = if avg_tvl > 0.0 { (daily_avg_fees * 365.0 / avg_tvl) * 100.0 } else { 0.0 };
            (fees_7d, volume_7d, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 {
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::Camelot,
            chain: Chain::Arbitrum,
            token0,
            token1,
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier,
            fee_rate_bps,
            tvl_usd: tvl,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr: 0.0,
            pool_address: pool.id.clone(),
            pool_id: Some(pool.id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(pool_address: &str) -> String {
        format!("https://app.camelot.exchange/pools/{}", pool_address)
    }
}

#[async_trait]
impl PoolIndexer for CamelotIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Camelot
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Arbitrum]
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        self.fetch_pools_for_chain(chain).await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.pool_address)
    }
}
