use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

const ORCA_API_URL: &str = "https://api.orca.so/v2/solana/pools?minTvl=10000&size=500";

// API v2 response structures

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Option<Vec<PoolInfo>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolInfo {
    address: Option<String>,
    #[serde(rename = "tokenA")]
    token_a: Option<TokenInfo>,
    #[serde(rename = "tokenB")]
    token_b: Option<TokenInfo>,
    fee_rate: Option<u32>,
    tvl_usdc: Option<String>,
    pool_type: Option<String>,
    stats: Option<PoolStats>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    symbol: Option<String>,
    #[allow(dead_code)]
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoolStats {
    #[serde(rename = "24h")]
    day: Option<PeriodStats>,
    #[serde(rename = "7d")]
    week: Option<PeriodStats>,
    #[serde(rename = "30d")]
    #[allow(dead_code)]
    month: Option<PeriodStats>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeriodStats {
    volume: Option<String>,
    fees: Option<String>,
    rewards: Option<String>,
    yield_over_tvl: Option<String>,
}

#[derive(Clone)]
pub struct OrcaIndexer {
    client: reqwest::Client,
}

impl OrcaIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Orca] Fetching pools from v2 API");

        let response = match self.client.get(ORCA_API_URL).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!("[Orca] API request failed: {}", e);
                return Ok(vec![]);
            }
        };

        let api_response: ApiResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Orca] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        let pools = match api_response.data {
            Some(pools) => pools,
            None => {
                tracing::warn!("[Orca] API returned no pools");
                return Ok(vec![]);
            }
        };

        tracing::info!("[Orca] Fetched {} pools", pools.len());

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(|p| self.parse_pool(p))
            .collect();

        tracing::info!("[Orca] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(&self, pool: PoolInfo) -> Option<PoolRate> {
        let address = pool.address.as_deref().unwrap_or_default();
        if address.is_empty() {
            return None;
        }

        let token0_symbol = pool.token_a.as_ref()
            .and_then(|t| t.symbol.as_deref())
            .unwrap_or("UNKNOWN");
        let token1_symbol = pool.token_b.as_ref()
            .and_then(|t| t.symbol.as_deref())
            .unwrap_or("UNKNOWN");

        let token0 = Asset::from_symbol(token0_symbol, "Orca");
        let token1 = Asset::from_symbol(token1_symbol, "Orca");

        let tvl_usd: f64 = pool.tvl_usdc.as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // feeRate is in hundredths of a bps (e.g., 400 = 4 bps = 0.04%, 3000 = 30 bps = 0.30%)
        let fee_rate_raw = pool.fee_rate.unwrap_or(0);
        let fee_rate_bps = fee_rate_raw / 100;
        let _fee_rate_decimal = fee_rate_raw as f64 / 1_000_000.0;
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        let stats = pool.stats.as_ref();

        let volume_24h: f64 = stats.and_then(|s| s.day.as_ref())
            .and_then(|d| d.volume.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let volume_7d: f64 = stats.and_then(|s| s.week.as_ref())
            .and_then(|d| d.volume.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        let fees_24h: f64 = stats.and_then(|s| s.day.as_ref())
            .and_then(|d| d.fees.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let fees_7d: f64 = stats.and_then(|s| s.week.as_ref())
            .and_then(|d| d.fees.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // yieldOverTvl is a decimal ratio for the period (fees+rewards / tvl)
        // Convert to annualized APR percentage
        let yield_24h: f64 = stats.and_then(|s| s.day.as_ref())
            .and_then(|d| d.yield_over_tvl.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let yield_7d: f64 = stats.and_then(|s| s.week.as_ref())
            .and_then(|d| d.yield_over_tvl.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // fee_apr = daily yield * 365 * 100 (to get percentage)
        let fee_apr_24h = yield_24h * 365.0 * 100.0;
        let fee_apr_7d = (yield_7d / 7.0) * 365.0 * 100.0;

        let rewards_24h: f64 = stats.and_then(|s| s.day.as_ref())
            .and_then(|d| d.rewards.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let rewards_apr = if tvl_usd > 0.0 {
            (rewards_24h / tvl_usd) * 365.0 * 100.0
        } else {
            0.0
        };

        let pool_type = match pool.pool_type.as_deref() {
            Some("whirlpool") => PoolType::ConcentratedLiquidity,
            _ => PoolType::ConcentratedLiquidity,
        };

        Some(PoolRate {
            protocol: Protocol::Orca,
            chain: Chain::Solana,
            token0,
            token1,
            pool_type,
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
            pool_address: address.to_string(),
            pool_id: Some(address.to_string()),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(pool_address: &str) -> String {
        format!("https://www.orca.so/pools/{}", pool_address)
    }
}

#[async_trait]
impl PoolIndexer for OrcaIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Orca
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Solana]
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        if !self.supported_chains().contains(chain) {
            return Ok(vec![]);
        }
        self.fetch_pools().await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.pool_address)
    }
}
