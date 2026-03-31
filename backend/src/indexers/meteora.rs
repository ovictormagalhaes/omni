use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

const METEORA_API_URL: &str = "https://dlmm-api.meteora.ag/pair/all_with_pagination";

#[derive(Clone)]
pub struct MeteoraIndexer {
    client: reqwest::Client,
}

// Meteora DLMM API response structures

#[derive(Debug, Deserialize)]
struct MeteoraResponse {
    pairs: Option<Vec<MeteoraPair>>,
}

#[derive(Debug, Deserialize)]
struct MeteoraPair {
    name: Option<String>,
    address: Option<String>,
    #[allow(dead_code)]
    mint_x: Option<String>,
    #[allow(dead_code)]
    mint_y: Option<String>,
    liquidity: Option<serde_json::Value>, // Can be string or number
    trade_volume_24h: Option<f64>,
    fees_24h: Option<f64>,
    base_fee_percentage: Option<serde_json::Value>, // Can be string or number
    #[allow(dead_code)]
    apr: Option<f64>,
    fee_apr: Option<f64>,
    reward_apr: Option<f64>,
}

impl MeteoraIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Meteora] Fetching DLMM pools from {}", METEORA_API_URL);

        let response = match self.client.get(METEORA_API_URL).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!("[Meteora] API request failed: {}", e);
                return Ok(vec![]);
            }
        };

        if !response.status().is_success() {
            tracing::warn!("[Meteora] API returned HTTP {}", response.status());
            return Ok(vec![]);
        }

        let api_response: MeteoraResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Meteora] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        let pairs = match api_response.pairs {
            Some(p) => p,
            None => {
                tracing::warn!("[Meteora] API returned no pairs");
                return Ok(vec![]);
            }
        };

        tracing::info!("[Meteora] Fetched {} DLMM pairs", pairs.len());

        let rates: Vec<PoolRate> = pairs
            .into_iter()
            .filter(|p| parse_liquidity(p) > 10000.0)
            .filter_map(|p| self.parse_pool(p))
            .collect();

        tracing::info!("[Meteora] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(&self, pair: MeteoraPair) -> Option<PoolRate> {
        let address = pair.address.as_deref().unwrap_or_default();
        if address.is_empty() {
            return None;
        }

        let name = pair.name.as_deref().unwrap_or_default();
        let (token0_str, token1_str) = parse_pair_name(name);
        if token0_str.is_empty() || token1_str.is_empty() {
            return None;
        }

        let token0 = Asset::from_symbol(&token0_str, "Meteora");
        let token1 = Asset::from_symbol(&token1_str, "Meteora");

        let tvl_usd = parse_liquidity(&pair);
        let volume_24h = pair.trade_volume_24h.unwrap_or(0.0);
        let fees_24h = pair.fees_24h.unwrap_or(0.0);

        // Fee APR: use API value, or calculate from fees/tvl
        let fee_apr_24h = pair.fee_apr.unwrap_or_else(|| {
            if tvl_usd > 0.0 && fees_24h > 0.0 {
                (fees_24h * 365.0 / tvl_usd) * 100.0
            } else {
                0.0
            }
        });

        let rewards_apr = pair.reward_apr.unwrap_or(0.0);

        // Parse base fee percentage (can be string like "0.25" or number)
        let base_fee_pct = match &pair.base_fee_percentage {
            Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.25),
            Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.25),
            _ => 0.25,
        };
        let fee_rate_bps = (base_fee_pct * 100.0).round() as u32; // 0.25% -> 25 bps
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        // Estimate 7d values (not available from Meteora API)
        let volume_7d = volume_24h * 7.0; // rough estimate
        let fees_7d = fees_24h * 7.0;
        let fee_apr_7d = fee_apr_24h; // use same as 24h as approximation

        Some(PoolRate {
            protocol: Protocol::Meteora,
            chain: Chain::Solana,
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
            pool_address: address.to_string(),
            pool_id: Some(address.to_string()),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(address: &str) -> String {
        format!("https://app.meteora.ag/dlmm/{}", address)
    }
}

#[async_trait]
impl PoolIndexer for MeteoraIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Meteora
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

/// Parse liquidity value which can be a string or number in the API response.
fn parse_liquidity(pair: &MeteoraPair) -> f64 {
    match &pair.liquidity {
        Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Parse pair name "TOKEN0-TOKEN1" into (token0, token1).
fn parse_pair_name(name: &str) -> (String, String) {
    let parts: Vec<&str> = name.splitn(2, '-').collect();
    if parts.len() == 2 {
        (parts[0].trim().to_string(), parts[1].trim().to_string())
    } else {
        (name.to_string(), String::new())
    }
}
