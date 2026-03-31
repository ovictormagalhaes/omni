use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

// ============================================================================
// SushiSwap V3 — The Graph Subgraph (Uniswap V3 fork)
// ============================================================================
// Direct data from on-chain via The Graph decentralized network.
// Subgraph schema is identical to Uniswap V3: pools, poolDayData, feeTier.
// Supported chains: Ethereum, Arbitrum, Base, Avalanche
// ============================================================================

/// SushiSwap V3 subgraph IDs on The Graph decentralized network
const SUBGRAPH_IDS: &[(&str, &str)] = &[
    ("ethereum", "2tGWMrDha4164KkFAfkU3rDCtuxGb4q1emXmFdLLzJ8x"),
    ("arbitrum", "9tSS5FaePZnjmnXnSKCCqKVLAqA6eGg6jA2oRojsXUbP"),
    ("base", "H6SjXCnZxJhaVHw4VDuXqtzWZ2JEBDvhwA3qysnUEjSV"),
    ("avalanche", "9WGqYsU8h1KVZeKz32663gFrbjVUNhBgmhRavMFqiSZz"),
];

#[derive(Clone)]
pub struct SushiSwapIndexer {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

// GraphQL response structures (same schema as Uniswap V3)

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
    #[serde(rename = "feeTier")]
    fee_tier: String,
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

impl SushiSwapIndexer {
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
        let chain_slug = Self::chain_to_slug(chain);
        let subgraph_id = match Self::get_subgraph_id(chain_slug) {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[SushiSwap] No THE_GRAPH_API_KEY configured, skipping");
                return Ok(vec![]);
            }
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, subgraph_id
        );

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

        tracing::info!("[SushiSwap] Fetching pools for {:?} from subgraph {}", chain, subgraph_id);

        let http_response = self.client
            .post(&url)
            .json(&query)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!(
                "[SushiSwap] HTTP {} for {:?}: {}",
                status, chain, &body[..body.len().min(200)]
            );
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[SushiSwap] Failed to parse response for {:?}: {} — body: {}",
                    chain, e, &body[..body.len().min(200)]
                );
                return Ok(vec![]);
            }
        };

        let pools = match response.data {
            Some(data) => data.pools,
            None => {
                tracing::warn!("[SushiSwap] No data returned for {:?}", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[SushiSwap] Fetched {} pools for {:?}", pools.len(), chain);

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(|p| Self::parse_pool(p, chain))
            .collect();

        tracing::info!("[SushiSwap] Parsed {} pools for {:?}", rates.len(), chain);
        Ok(rates)
    }

    fn parse_pool(pool: SubgraphPool, chain: &Chain) -> Option<PoolRate> {
        let tvl: f64 = pool.total_value_locked_usd.parse().unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        let fee_tier_raw: u32 = pool.fee_tier.parse().unwrap_or(3000);
        let fee_tier = FeeTier::from_uniswap_fee(fee_tier_raw);
        let fee_rate_bps = fee_tier.to_bps();

        let token0 = Asset::from_symbol(&pool.token0.symbol, "SushiSwap");
        let token1 = Asset::from_symbol(&pool.token1.symbol, "SushiSwap");

        // 24h metrics from most recent poolDayData
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
            protocol: Protocol::SushiSwap,
            chain: chain.clone(),
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

    fn chain_to_slug(chain: &Chain) -> &'static str {
        match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Avalanche => "avalanche",
            _ => "",
        }
    }

    fn get_subgraph_id(chain_slug: &str) -> Option<&'static str> {
        SUBGRAPH_IDS.iter()
            .find(|(slug, _)| *slug == chain_slug)
            .map(|(_, id)| *id)
    }

    pub fn get_pool_url(chain: &Chain, pool_address: &str) -> String {
        let chain_id = match chain {
            Chain::Ethereum => "1",
            Chain::Arbitrum => "42161",
            Chain::Base => "8453",
            Chain::Avalanche => "43114",
            _ => "1",
        };
        format!(
            "https://www.sushi.com/pool/{}/{}",
            chain_id, pool_address
        )
    }
}

#[async_trait]
impl PoolIndexer for SushiSwapIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::SushiSwap
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::Arbitrum, Chain::Base, Chain::Avalanche]
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        if !self.supported_chains().contains(chain) {
            return Ok(vec![]);
        }
        self.fetch_pools_for_chain(chain).await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.chain, &pool.pool_address)
    }
}
