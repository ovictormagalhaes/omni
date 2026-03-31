use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

/// Uniswap V3 subgraph IDs on The Graph decentralized network
/// Verified working as of 2026-03-25
pub const SUBGRAPH_IDS: &[(&str, &str)] = &[
    ("ethereum", "5zvR82QoaXYFyDEKLZ9t6v9adgnptxYpKpSbxtgVENFV"),
    ("arbitrum", "FbCGRftH4a3yZugY7TnbYgPJVEv2LvMT6oF1fxPe9aJM"),
    ("base", "HMuAwufqZ1YCRmzL2SfHTVkzZovC9VL2UAKhjvRqKiR1"),
    ("polygon", "3hCPRGf4z88VC5rsBKU5AA9FBBq5nF3jbKJG7VZCbhjm"),
    ("celo", "ESdrTJ3twMwWVoQ1hUE2u7PugEHX3QkenudD6aXCkDQ4"),
];

#[derive(Clone)]
pub struct UniswapV3Indexer {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

// GraphQL response structures

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
    #[serde(rename = "volumeUSD")]
    #[allow(dead_code)]
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

// Merkl API response structures (for rewards APR)
#[derive(Debug, Deserialize)]
struct MerklOpportunity {
    identifier: Option<String>,
    name: Option<String>,
    apr: Option<f64>,
    status: Option<String>,
}

impl UniswapV3Indexer {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            graph_api_key,
        }
    }

    /// Fetch Merkl reward APRs for Uniswap V3 pools on a given chain.
    /// Returns a HashMap of pool_address (lowercase) -> rewards_apr (percentage).
    async fn fetch_merkl_rewards(&self, chain: &Chain) -> HashMap<String, f64> {
        let chain_id = Self::chain_to_chain_id(chain);
        if chain_id == 0 {
            return HashMap::new();
        }

        // Merkl v4 API: action=POOL, items=100, filter UniswapV3 client-side by name
        let url = format!(
            "https://api.merkl.xyz/v4/opportunities?chainId={}&action=POOL&items=100",
            chain_id
        );

        tracing::debug!("[Merkl] Fetching rewards for chain {:?} (chainId={})", chain, chain_id);

        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Merkl] Failed to fetch rewards for {:?}: {}", chain, e);
                return HashMap::new();
            }
        };

        let opportunities: Vec<MerklOpportunity> = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Merkl] Failed to parse rewards for {:?}: {}", chain, e);
                return HashMap::new();
            }
        };

        let mut rewards_map = HashMap::new();
        for opp in opportunities {
            let is_live = opp.status.as_deref() == Some("LIVE");
            if !is_live {
                continue;
            }
            // Filter for UniswapV3 pools by name (Merkl v4 doesn't support protocol filter)
            let is_uni_v3 = opp.name.as_deref()
                .map(|n| n.contains("UniswapV3") || n.contains("Uniswap V3"))
                .unwrap_or(false);
            if !is_uni_v3 {
                continue;
            }
            if let (Some(identifier), Some(apr)) = (opp.identifier, opp.apr) {
                if apr > 0.0 {
                    rewards_map.insert(identifier.to_lowercase(), apr);
                }
            }
        }

        tracing::info!("[Merkl] Found {} active reward pools for {:?}", rewards_map.len(), chain);
        rewards_map
    }

    fn chain_to_chain_id(chain: &Chain) -> u64 {
        match chain {
            Chain::Ethereum => 1,
            Chain::Arbitrum => 42161,
            Chain::Base => 8453,
            Chain::Polygon => 137,
            Chain::Celo => 42220,
            _ => 0,
        }
    }

    pub async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        let chain_slug = Self::chain_to_slug(chain);
        let subgraph_id = match Self::get_subgraph_id(chain_slug) {
            Some(id) => id,
            None => {
                tracing::debug!("[Uniswap V3] No subgraph for chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[Uniswap V3] No THE_GRAPH_API_KEY configured, skipping");
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

        tracing::info!("[Uniswap V3] Fetching pools for {:?} from subgraph {}", chain, subgraph_id);

        let http_response = self.client
            .post(&url)
            .json(&query)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!(
                "[Uniswap V3] HTTP {} for {:?}: {}",
                status, chain, &body[..body.len().min(200)]
            );
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[Uniswap V3] Failed to parse response for {:?}: {} — body starts with: {}",
                    chain, e, &body[..body.len().min(200)]
                );
                return Ok(vec![]);
            }
        };

        let pools = match response.data {
            Some(data) => data.pools,
            None => {
                tracing::warn!("[Uniswap V3] No data returned for {:?}", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[Uniswap V3] Fetched {} pools for {:?}", pools.len(), chain);

        let mut rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(|p| self.parse_pool(p, chain))
            .collect();

        tracing::info!("[Uniswap V3] Parsed {} pools for {:?}", rates.len(), chain);

        // Fetch Merkl rewards and merge into pool rates
        let merkl_rewards = self.fetch_merkl_rewards(chain).await;
        if !merkl_rewards.is_empty() {
            let mut matched = 0;
            for rate in &mut rates {
                if let Some(apr) = merkl_rewards.get(&rate.pool_address.to_lowercase()) {
                    rate.rewards_apr = *apr;
                    matched += 1;
                }
            }
            tracing::info!("[Uniswap V3] Merged Merkl rewards into {} pools for {:?}", matched, chain);
        }

        Ok(rates)
    }

    fn parse_pool(&self, pool: SubgraphPool, chain: &Chain) -> Option<PoolRate> {
        let tvl: f64 = pool.total_value_locked_usd.parse().unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        let fee_tier_raw: u32 = pool.fee_tier.parse().unwrap_or(3000);
        let fee_tier = FeeTier::from_uniswap_fee(fee_tier_raw);
        let fee_rate_bps = fee_tier.to_bps();

        let token0 = Asset::from_symbol(&pool.token0.symbol, "Uniswap");
        let token1 = Asset::from_symbol(&pool.token1.symbol, "Uniswap");

        // Calculate fee APR from poolDayData
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
            // Extrapolate to 7 days so volume_7d and fees_7d represent full-week estimates
            let fees_7d = daily_avg_fees * 7.0;
            let volume_7d = daily_avg_volume * 7.0;
            let apr = if avg_tvl > 0.0 { (daily_avg_fees * 365.0 / avg_tvl) * 100.0 } else { 0.0 };
            (fees_7d, volume_7d, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        // Reject pools with extreme APR values (likely wash trading or data errors)
        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 {
            tracing::warn!(
                "[Uniswap V3] Rejecting pool {}-{} ({}) with extreme APR: fee_24h={:.2}%, fee_7d={:.2}%",
                pool.token0.symbol, pool.token1.symbol, pool.id, fee_apr_24h, fee_apr_7d
            );
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::Uniswap,
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
            Chain::Polygon => "polygon",
            Chain::Celo => "celo",
            _ => "",
        }
    }

    fn get_subgraph_id(chain_slug: &str) -> Option<&'static str> {
        SUBGRAPH_IDS.iter()
            .find(|(slug, _)| *slug == chain_slug)
            .map(|(_, id)| *id)
    }

    /// Chains supported by Uniswap V3 subgraphs
    pub fn supported_chains() -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Celo,
        ]
    }

    pub fn get_pool_url(chain: &Chain, pool_address: &str) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum-one",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Celo => "celo",
            _ => "ethereum",
        };
        format!("https://app.uniswap.org/explore/pools/{}/{}", chain_slug, pool_address)
    }
}

#[async_trait]
impl PoolIndexer for UniswapV3Indexer {
    fn protocol(&self) -> Protocol {
        Protocol::Uniswap
    }

    fn supported_chains(&self) -> Vec<Chain> {
        Self::supported_chains()
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        self.fetch_pools(chain).await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.chain, &pool.pool_address)
    }
}
