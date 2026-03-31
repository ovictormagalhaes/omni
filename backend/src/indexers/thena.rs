use anyhow::Result;
use chrono::Utc;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use crate::indexers::defillama_pools::{self, DefiLlamaCache};

// ============================================================================
// Thena V3 (Fusion) - DeFiLlama Integration
// ============================================================================
// BSC-native DEX (ve(3,3) + concentrated liquidity). Uses DeFiLlama yields API.
// DeFiLlama project name: "thena-fusion"
// Supported chains: BSC
// ============================================================================

#[derive(Clone)]
pub struct ThenaIndexer {
    client: reqwest::Client,
    defillama_cache: Option<DefiLlamaCache>,
}

impl ThenaIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            defillama_cache: None,
        }
    }

    pub fn with_cache(mut self, cache: DefiLlamaCache) -> Self {
        self.defillama_cache = Some(cache);
        self
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Thena] Fetching pools from DeFiLlama yields API");

        let all_pools = if let Some(ref cache) = self.defillama_cache {
            match cache.get_pools_by_project(&["thena-fusion"]).await {
                Ok(pools) => pools,
                Err(e) => {
                    tracing::warn!("[Thena] Failed to get cached DeFiLlama pools: {}", e);
                    return Ok(vec![]);
                }
            }
        } else {
            match defillama_pools::fetch_defillama_pools(&self.client).await {
                Ok(pools) => pools,
                Err(e) => {
                    tracing::warn!("[Thena] Failed to fetch DeFiLlama pools: {}", e);
                    return Ok(vec![]);
                }
            }
        };

        let thena_pools: Vec<_> = all_pools
            .into_iter()
            .filter(|p| p.project.as_deref() == Some("thena-fusion"))
            .collect();

        tracing::info!("[Thena] Found {} thena-v3 pools", thena_pools.len());

        let rates: Vec<PoolRate> = thena_pools
            .into_iter()
            .filter(|p| p.tvl_usd.unwrap_or(0.0) > 10000.0)
            .filter(|p| p.volume_usd_1d.unwrap_or(0.0) > 0.0)
            .filter_map(|p| self.parse_pool(p))
            .collect();

        tracing::info!("[Thena] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(&self, pool: defillama_pools::DefiLlamaPool) -> Option<PoolRate> {
        let chain_str = pool.chain.as_deref().unwrap_or_default();
        let chain = defillama_pools::parse_chain(chain_str)?;

        if chain != Chain::BSC {
            return None;
        }

        let symbol = pool.symbol.as_deref().unwrap_or_default();
        let (token0_str, token1_str) = defillama_pools::parse_symbol(symbol);

        if token0_str.is_empty() || token1_str.is_empty() {
            return None;
        }

        let token0 = Asset::from_symbol(&token0_str, "Thena");
        let token1 = Asset::from_symbol(&token1_str, "Thena");

        let pool_id = pool.pool.as_deref().unwrap_or_default().to_string();
        if pool_id.is_empty() {
            return None;
        }

        let tvl_usd = pool.tvl_usd.unwrap_or(0.0);
        let fee_rate_bps: u32 = 30;
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        let rewards_apr = pool.apy_reward.unwrap_or(0.0);

        let volume_24h = pool.volume_usd_1d.unwrap_or(0.0);
        let volume_7d = pool.volume_usd_7d.unwrap_or(0.0);
        let fees_24h = volume_24h * fee_rate_bps as f64 / 10000.0;
        let fees_7d = volume_7d * fee_rate_bps as f64 / 10000.0;

        // Calculate manual fee APR from volume data as a sanity baseline
        let manual_fee_apr_24h = if tvl_usd > 0.0 && volume_24h > 0.0 {
            (fees_24h * 365.0 / tvl_usd) * 100.0
        } else {
            0.0
        };

        // Use DeFiLlama's apyBase but cross-validate: if it diverges >3x from manual calc, prefer manual
        let fee_apr_24h = match pool.apy_base {
            Some(base) if base > 0.0 && manual_fee_apr_24h > 0.0 && base > manual_fee_apr_24h * 3.0 => {
                tracing::warn!(
                    "[Thena] apyBase ({:.2}%) diverges from manual calc ({:.2}%) for {}, using manual",
                    base, manual_fee_apr_24h, symbol
                );
                manual_fee_apr_24h
            }
            Some(base) if base > 0.0 => base,
            _ => manual_fee_apr_24h,
        };

        let fee_apr_7d = if tvl_usd > 0.0 && volume_7d > 0.0 {
            let manual = {
                let daily_avg_fees = fees_7d / 7.0;
                (daily_avg_fees * 365.0 / tvl_usd) * 100.0
            };
            match pool.apy_base {
                Some(base) if base > 0.0 && manual > base * 3.0 => base,
                _ => manual,
            }
        } else {
            0.0
        };

        // Reject pools with absurd APR values (data errors)
        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 || rewards_apr > 10000.0 {
            tracing::warn!(
                "[Thena] Rejecting pool {} with extreme APR: fee_24h={:.2}%, fee_7d={:.2}%, rewards={:.2}%",
                symbol, fee_apr_24h, fee_apr_7d, rewards_apr
            );
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::Thena,
            chain,
            token0,
            token1,
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier,
            fee_rate_bps,
            tvl_usd,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr,
            pool_address: pool_id.clone(),
            pool_id: Some(pool_id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(token0: &str, token1: &str) -> String {
        format!(
            "https://www.thena.fi/liquidity/manage?token0={}&token1={}",
            token0, token1
        )
    }
}
