use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::PoolIndexer;
use crate::models::{Asset, Chain, FeeTier, PoolRate, PoolType, Protocol};

// ============================================================================
// Maverick V2 - Official API Integration
// ============================================================================
// Directional liquidity AMM. Uses official Maverick API.
// API: https://api.mav.xyz/api/v4/pools/{chainId}
// Supported chains: Ethereum (1), BSC (56), Base (8453), zkSync (324)
// ============================================================================

const MAVERICK_API_BASE: &str = "https://api.mav.xyz/api/v4";

const CHAIN_IDS: &[(Chain, u64)] = &[
    (Chain::Ethereum, 1),
    (Chain::BSC, 56),
    (Chain::Base, 8453),
    (Chain::ZkSync, 324),
];

// ── API response structures ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MaverickPoolsResponse {
    #[serde(default)]
    pools: Vec<MaverickPool>,
}

#[derive(Debug, Deserialize)]
struct MaverickPool {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    name: Option<String>,
    #[serde(default, rename = "tokenA")]
    token_a: Option<MaverickToken>,
    #[serde(default, rename = "tokenB")]
    token_b: Option<MaverickToken>,
    #[serde(default)]
    tvl: Option<MaverickAmount>,
    #[serde(default)]
    volume: Option<MaverickAmount>,
    #[serde(default)]
    fee: Option<f64>,
    #[serde(default, rename = "feeVolume")]
    fee_volume: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MaverickToken {
    #[serde(default)]
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MaverickAmount {
    #[serde(default)]
    amount: Option<f64>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Clone)]
pub struct MaverickIndexer {
    client: reqwest::Client,
}

impl MaverickIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Maverick] Fetching pools from official API");

        let mut all_rates = Vec::new();

        for (chain, chain_id) in CHAIN_IDS {
            let url = format!("{}/pools/{}", MAVERICK_API_BASE, chain_id);

            let response = match self.client.get(&url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!("[Maverick] Failed to fetch pools for {:?}: {}", chain, e);
                    continue;
                }
            };

            if !response.status().is_success() {
                tracing::warn!(
                    "[Maverick] API returned {} for {:?}",
                    response.status(),
                    chain
                );
                continue;
            }

            let pools_resp: MaverickPoolsResponse = match response.json().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("[Maverick] Failed to parse response for {:?}: {}", chain, e);
                    continue;
                }
            };

            tracing::debug!(
                "[Maverick] Found {} pools on {:?}",
                pools_resp.pools.len(),
                chain
            );

            let rates: Vec<PoolRate> = pools_resp
                .pools
                .into_iter()
                .filter(|p| p.tvl.as_ref().and_then(|t| t.amount).unwrap_or(0.0) > 10000.0)
                .filter_map(|p| self.parse_pool(p, chain))
                .collect();

            all_rates.extend(rates);
        }

        tracing::info!(
            "[Maverick] Parsed {} pools total after filtering",
            all_rates.len()
        );
        Ok(all_rates)
    }

    fn parse_pool(&self, pool: MaverickPool, chain: &Chain) -> Option<PoolRate> {
        let token0_str = pool.token_a.as_ref()?.symbol.as_deref()?;
        let token1_str = pool.token_b.as_ref()?.symbol.as_deref()?;

        if token0_str.is_empty() || token1_str.is_empty() {
            return None;
        }

        let token0 = Asset::from_symbol(token0_str, "Maverick");
        let token1 = Asset::from_symbol(token1_str, "Maverick");

        let pool_id = pool.id.unwrap_or_default();
        if pool_id.is_empty() {
            return None;
        }

        let tvl_usd = pool.tvl.as_ref().and_then(|t| t.amount).unwrap_or(0.0);

        // fee is decimal (e.g. 0.003 = 0.3% = 30 bps)
        let fee_decimal = pool.fee.unwrap_or(0.001);
        let fee_rate_bps = (fee_decimal * 10000.0).round() as u32;
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        let volume_24h = pool.volume.as_ref().and_then(|v| v.amount).unwrap_or(0.0);
        let fees_24h = pool.fee_volume.unwrap_or(volume_24h * fee_decimal);

        // Calculate APR from fees and TVL
        let fee_apr_24h = if tvl_usd > 0.0 && fees_24h > 0.0 {
            (fees_24h * 365.0 / tvl_usd) * 100.0
        } else {
            0.0
        };

        if fee_apr_24h > 10000.0 {
            tracing::warn!(
                "[Maverick] Rejecting pool {}-{} with extreme APR: {:.2}%",
                token0_str,
                token1_str,
                fee_apr_24h
            );
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::Maverick,
            chain: chain.clone(),
            token0,
            token1,
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier,
            fee_rate_bps,
            tvl_usd,
            volume_24h_usd: volume_24h,
            volume_7d_usd: 0.0,
            fees_24h_usd: fees_24h,
            fees_7d_usd: 0.0,
            fee_apr_24h,
            fee_apr_7d: 0.0,
            rewards_apr: 0.0,
            pool_address: pool_id.clone(),
            pool_id: Some(pool_id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(chain: &Chain) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::BSC => "bnb",
            Chain::Base => "base",
            Chain::ZkSync => "zksync",
            _ => "ethereum",
        };
        format!("https://app.mav.xyz/pools?chain={}", chain_slug)
    }
}

#[async_trait]
impl PoolIndexer for MaverickIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Maverick
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::BSC, Chain::Base, Chain::ZkSync]
    }

    async fn fetch_pools(&self, _chain: &Chain) -> Result<Vec<PoolRate>> {
        self.fetch_pools().await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_pools() {
        let indexer = MaverickIndexer::new();
        let result = indexer.fetch_pools().await;
        assert!(
            result.is_ok(),
            "Failed to fetch Maverick pools: {:?}",
            result.err()
        );

        let pools = result.unwrap();
        println!("Maverick: {} pools from official API", pools.len());
        assert!(!pools.is_empty(), "Maverick should return pools");

        for pool in pools.iter().take(5) {
            println!(
                "  {}/{} on {:?}: TVL ${:.0}, Fee APR {:.2}%, fee={} bps",
                pool.token0.symbol(),
                pool.token1.symbol(),
                pool.chain,
                pool.tvl_usd,
                pool.fee_apr_24h,
                pool.fee_rate_bps
            );
        }
    }
}
