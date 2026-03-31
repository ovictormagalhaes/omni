use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

/// Uniswap V4 indexer using Uniswap's official GraphQL API (interface.gateway.uniswap.org).
/// V4 uses a singleton PoolManager contract with hooks support.
#[derive(Clone)]
pub struct UniswapV4Indexer {
    client: reqwest::Client,
}

// GraphQL response structures for Uniswap's API

#[derive(Debug, Deserialize)]
struct V4GraphQLResponse {
    data: Option<V4GraphQLData>,
    errors: Option<Vec<V4GraphQLError>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct V4GraphQLData {
    #[serde(rename = "topV4Pools")]
    top_v4_pools: Vec<V4Pool>,
}

#[derive(Debug, Deserialize)]
struct V4GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct V4Pool {
    #[serde(rename = "poolId")]
    pool_id: String,
    #[serde(rename = "feeTier")]
    fee_tier: f64,
    token0: V4Token,
    token1: V4Token,
    #[serde(rename = "totalLiquidity")]
    total_liquidity: AmountValue,
    #[serde(rename = "cumulativeVolume")]
    cumulative_volume: AmountValue,
    volume7d: Option<AmountValue>,
}

#[derive(Debug, Deserialize)]
struct V4Token {
    symbol: String,
    #[allow(dead_code)]
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AmountValue {
    value: f64,
}

// Merkl API structures (same as V3)
#[derive(Debug, Deserialize)]
struct MerklOpportunity {
    identifier: Option<String>,
    name: Option<String>,
    apr: Option<f64>,
    status: Option<String>,
}

impl UniswapV4Indexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    pub async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        let chain_name = match Self::chain_to_api_name(chain) {
            Some(name) => name,
            None => {
                tracing::debug!("[Uniswap V4] Chain {:?} not supported, skipping", chain);
                return Ok(vec![]);
            }
        };

        let query = serde_json::json!({
            "query": format!(
                r#"{{
                    topV4Pools(first: 100, chain: {chain_name}) {{
                        poolId
                        feeTier
                        token0 {{ symbol address }}
                        token1 {{ symbol address }}
                        totalLiquidity {{ value }}
                        cumulativeVolume(duration: DAY) {{ value }}
                        volume7d: cumulativeVolume(duration: WEEK) {{ value }}
                    }}
                }}"#
            )
        });

        tracing::info!("[Uniswap V4] Fetching pools for {:?}", chain);

        let http_response = self.client
            .post("https://interface.gateway.uniswap.org/v1/graphql")
            .header("Content-Type", "application/json")
            .header("Origin", "https://app.uniswap.org")
            .json(&query)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!(
                "[Uniswap V4] HTTP {} for {:?}: {}",
                status, chain, &body[..body.len().min(200)]
            );
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: V4GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[Uniswap V4] Failed to parse response for {:?}: {} — body: {}",
                    chain, e, &body[..body.len().min(200)]
                );
                return Ok(vec![]);
            }
        };

        if let Some(errors) = &response.errors {
            for err in errors {
                tracing::warn!("[Uniswap V4] GraphQL error for {:?}: {}", chain, err.message);
            }
        }

        let pools = match response.data {
            Some(data) => data.top_v4_pools,
            None => {
                tracing::warn!("[Uniswap V4] No data returned for {:?}", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[Uniswap V4] Fetched {} pools for {:?}", pools.len(), chain);

        let mut rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(|p| self.parse_pool(p, chain))
            .collect();

        tracing::info!("[Uniswap V4] Parsed {} pools for {:?}", rates.len(), chain);

        // Fetch Merkl rewards and merge
        let merkl_rewards = self.fetch_merkl_rewards(chain).await;
        if !merkl_rewards.is_empty() {
            let mut matched = 0;
            for rate in &mut rates {
                if let Some(apr) = merkl_rewards.get(&rate.pool_address.to_lowercase()) {
                    rate.rewards_apr = *apr;
                    matched += 1;
                }
            }
            if matched > 0 {
                tracing::info!("[Uniswap V4] Merged Merkl rewards into {} pools for {:?}", matched, chain);
            }
        }

        Ok(rates)
    }

    fn parse_pool(&self, pool: V4Pool, chain: &Chain) -> Option<PoolRate> {
        let tvl = pool.total_liquidity.value;
        if tvl <= 0.0 {
            return None;
        }

        // V4 feeTier is in hundredths of a bip (like V3: 100 = 0.01%, 500 = 0.05%, 3000 = 0.30%)
        // But it can also be dynamic/custom fees
        let fee_tier_raw = pool.fee_tier as u32;
        let fee_tier = FeeTier::from_uniswap_fee(fee_tier_raw);
        let fee_rate_bps = fee_tier.to_bps();

        let token0 = Asset::from_symbol(&pool.token0.symbol, "Uniswap");
        let token1 = Asset::from_symbol(&pool.token1.symbol, "Uniswap");

        let volume_24h = pool.cumulative_volume.value;
        let volume_7d = pool.volume7d.map(|v| v.value).unwrap_or(0.0);

        // Fee APR: (volume * fee_rate / tvl) * 365 * 100
        let fee_rate = fee_rate_bps as f64 / 10_000.0;
        let fee_apr_24h = if tvl > 0.0 {
            (volume_24h * fee_rate / tvl) * 365.0 * 100.0
        } else {
            0.0
        };

        let daily_avg_volume_7d = volume_7d / 7.0;
        let fee_apr_7d = if tvl > 0.0 {
            (daily_avg_volume_7d * fee_rate / tvl) * 365.0 * 100.0
        } else {
            0.0
        };

        let fees_24h = volume_24h * fee_rate;
        let fees_7d = volume_7d * fee_rate;

        // Reject pools with extreme APR values (likely wash trading or data errors)
        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 {
            tracing::warn!(
                "[Uniswap V4] Rejecting pool {}-{} ({}) with extreme APR: fee_24h={:.2}%, fee_7d={:.2}%",
                pool.token0.symbol, pool.token1.symbol, pool.pool_id, fee_apr_24h, fee_apr_7d
            );
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::UniswapV4,
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
            pool_address: pool.pool_id.clone(),
            pool_id: Some(pool.pool_id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    /// Fetch Merkl reward APRs for Uniswap V4 pools on a given chain.
    async fn fetch_merkl_rewards(&self, chain: &Chain) -> HashMap<String, f64> {
        let chain_id = Self::chain_to_chain_id(chain);
        if chain_id == 0 {
            return HashMap::new();
        }

        let url = format!(
            "https://api.merkl.xyz/v4/opportunities?chainId={}&action=POOL&items=100",
            chain_id
        );

        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Merkl/V4] Failed to fetch rewards for {:?}: {}", chain, e);
                return HashMap::new();
            }
        };

        let opportunities: Vec<MerklOpportunity> = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Merkl/V4] Failed to parse rewards for {:?}: {}", chain, e);
                return HashMap::new();
            }
        };

        let mut rewards_map = HashMap::new();
        for opp in opportunities {
            if opp.status.as_deref() != Some("LIVE") {
                continue;
            }
            let is_uni_v4 = opp.name.as_deref()
                .map(|n| n.contains("UniswapV4") || n.contains("Uniswap V4"))
                .unwrap_or(false);
            if !is_uni_v4 {
                continue;
            }
            if let (Some(identifier), Some(apr)) = (opp.identifier, opp.apr) {
                if apr > 0.0 {
                    rewards_map.insert(identifier.to_lowercase(), apr);
                }
            }
        }

        if !rewards_map.is_empty() {
            tracing::info!("[Merkl/V4] Found {} active reward pools for {:?}", rewards_map.len(), chain);
        }
        rewards_map
    }

    fn chain_to_api_name(chain: &Chain) -> Option<&'static str> {
        match chain {
            Chain::Ethereum => Some("ETHEREUM"),
            Chain::Base => Some("BASE"),
            Chain::Arbitrum => Some("ARBITRUM"),
            Chain::Polygon => Some("POLYGON"),
            Chain::Optimism => Some("OPTIMISM"),
            _ => None,
        }
    }

    fn chain_to_chain_id(chain: &Chain) -> u64 {
        match chain {
            Chain::Ethereum => 1,
            Chain::Arbitrum => 42161,
            Chain::Base => 8453,
            Chain::Polygon => 137,
            Chain::Optimism => 10,
            _ => 0,
        }
    }

    /// Chains supported by Uniswap V4
    pub fn supported_chains() -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Base,
            Chain::Arbitrum,
            Chain::Polygon,
            Chain::Optimism,
        ]
    }

    pub fn get_pool_url(chain: &Chain, pool_id: &str) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum-one",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Optimism => "optimism",
            _ => "ethereum",
        };
        format!("https://app.uniswap.org/explore/pools/{}/{}", chain_slug, pool_id)
    }
}

#[async_trait]
impl PoolIndexer for UniswapV4Indexer {
    fn protocol(&self) -> Protocol {
        Protocol::UniswapV4
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
