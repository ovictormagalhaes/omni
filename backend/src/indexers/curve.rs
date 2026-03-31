use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

#[derive(Clone)]
pub struct CurveIndexer {
    client: reqwest::Client,
}

// ── Curve Pools API response structures ──────────────────────────────

#[derive(Debug, Deserialize)]
struct PoolsApiResponse {
    data: Option<PoolsApiData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolsApiData {
    pool_data: Vec<CurvePool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurvePool {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    coins: Vec<CurveCoin>,
    #[serde(default)]
    usd_total: Option<f64>,
    #[serde(default)]
    gauge_rewards: Option<Vec<GaugeReward>>,
    #[serde(default)]
    gauge_crv_apy: Option<Vec<Option<f64>>>,
    #[serde(default)]
    pool_urls: Option<PoolUrls>,
}

#[derive(Debug, Deserialize)]
struct CurveCoin {
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default, rename = "usdPrice")]
    _usd_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GaugeReward {
    #[serde(default)]
    apy: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoolUrls {
    #[serde(default)]
    swap: Option<Vec<String>>,
}

// ── Curve Volumes API response structures ────────────────────────────

#[derive(Debug, Deserialize)]
struct VolumesApiResponse {
    data: Option<VolumesApiData>,
}

#[derive(Debug, Deserialize)]
struct VolumesApiData {
    #[serde(default)]
    pools: Vec<VolumePool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumePool {
    address: String,
    #[serde(default, rename = "volumeUSD")]
    volume_usd: Option<f64>,
    #[serde(default)]
    latest_daily_apy_pcent: Option<f64>,
    #[serde(default)]
    latest_weekly_apy_pcent: Option<f64>,
}

// ── Implementation ───────────────────────────────────────────────────

impl CurveIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
        }
    }

    pub async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        let slug = match chain_to_slug(chain) {
            Some(s) => s,
            None => {
                tracing::debug!("[Curve] Unsupported chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        // Fetch pools metadata and volume data in parallel
        let pools_url = format!("https://api.curve.fi/v1/getPools/all/{}", slug);
        let volumes_url = format!("https://api.curve.fi/v1/getVolumes/{}", slug);

        tracing::info!("[Curve] Fetching pools for {:?} from {} and {}", chain, pools_url, volumes_url);

        let (pools_resp, volumes_resp) = tokio::join!(
            self.client.get(&pools_url).send(),
            self.client.get(&volumes_url).send(),
        );

        // Parse pools response
        let pools: Vec<CurvePool> = match pools_resp {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!("[Curve] Pools API HTTP {} for {:?}", resp.status(), chain);
                    return Ok(vec![]);
                }
                let body = match resp.text().await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("[Curve] Failed to read pools body for {:?}: {}", chain, e);
                        return Ok(vec![]);
                    }
                };
                tracing::debug!("[Curve] Pools response for {:?}: {} bytes", chain, body.len());
                match serde_json::from_str::<PoolsApiResponse>(&body) {
                    Ok(data) => data.data.map(|d| d.pool_data).unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!("[Curve] Failed to parse pools for {:?}: {} — body starts: {}", chain, e, &body[..body.len().min(200)]);
                        return Ok(vec![]);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("[Curve] Failed to fetch pools for {:?}: {}", chain, e);
                return Ok(vec![]);
            }
        };

        // Parse volumes response into a lookup map by address (lowercased)
        let volume_map: HashMap<String, VolumePool> = match volumes_resp {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!("[Curve] Volumes API HTTP {} for {:?}", resp.status(), chain);
                    HashMap::new()
                } else {
                    match resp.json::<VolumesApiResponse>().await {
                        Ok(data) => {
                            data.data
                                .map(|d| {
                                    d.pools
                                        .into_iter()
                                        .map(|v| (v.address.to_lowercase(), v))
                                        .collect()
                                })
                                .unwrap_or_default()
                        }
                        Err(e) => {
                            tracing::warn!("[Curve] Failed to parse volumes response for {:?}: {}", chain, e);
                            HashMap::new()
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("[Curve] Failed to fetch volumes for {:?}: {}", chain, e);
                HashMap::new()
            }
        };

        tracing::info!(
            "[Curve] Fetched {} pools and {} volume entries for {:?}",
            pools.len(),
            volume_map.len(),
            chain
        );

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter(|p| p.usd_total.unwrap_or(0.0) > 10000.0)
            .filter_map(|p| self.parse_pool(p, &volume_map, chain))
            .collect();

        tracing::info!("[Curve] Parsed {} pools for {:?} after filtering", rates.len(), chain);
        Ok(rates)
    }

    fn parse_pool(
        &self,
        pool: CurvePool,
        volume_map: &HashMap<String, VolumePool>,
        chain: &Chain,
    ) -> Option<PoolRate> {
        let address = pool.address.as_deref().unwrap_or("").to_string();
        if address.is_empty() {
            return None;
        }

        let tvl = pool.usd_total.unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        // Determine token0 / token1
        let (token0_symbol, token1_symbol) = if pool.coins.len() == 2 {
            let t0 = pool.coins[0]
                .symbol
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_string();
            let t1 = pool.coins[1]
                .symbol
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_string();
            (t0, t1)
        } else if pool.coins.len() >= 3 {
            let t0 = pool.coins[0]
                .symbol
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_string();
            let t1 = pool
                .name
                .clone()
                .unwrap_or_else(|| format!("{}pool", pool.coins.len()));
            (t0, t1)
        } else {
            // Single coin or empty pool — skip
            return None;
        };

        let token0 = Asset::from_symbol(&token0_symbol, "Curve");
        let token1 = Asset::from_symbol(&token1_symbol, "Curve");

        // Volume and APY data from the volumes endpoint
        let vol_data = volume_map.get(&address.to_lowercase());
        let volume_24h = vol_data.and_then(|v| v.volume_usd).unwrap_or(0.0);
        let fee_apr_24h = vol_data.and_then(|v| v.latest_daily_apy_pcent).unwrap_or(0.0);
        let fee_apr_7d = vol_data.and_then(|v| v.latest_weekly_apy_pcent).unwrap_or(0.0);

        // Estimate fees from volume (0.04% typical Curve fee)
        let fees_24h = volume_24h * 0.0004;

        // Rewards APR: sum of gauge rewards APY + gaugeCrvApy[0] * 100
        let gauge_rewards_apy: f64 = pool
            .gauge_rewards
            .as_ref()
            .map(|rewards| {
                rewards
                    .iter()
                    .filter_map(|r| r.apy)
                    .sum::<f64>()
            })
            .unwrap_or(0.0);

        let crv_apy: f64 = pool
            .gauge_crv_apy
            .as_ref()
            .and_then(|apys| apys.first().copied().flatten())
            .map(|v| v * 100.0)
            .unwrap_or(0.0);

        let rewards_apr = gauge_rewards_apy + crv_apy;

        // Pool URL
        let _pool_url = pool
            .pool_urls
            .as_ref()
            .and_then(|urls| urls.swap.as_ref())
            .and_then(|swap_urls| swap_urls.first().cloned())
            .unwrap_or_else(|| Self::get_pool_url(chain, &address));

        // Curve uses 0.04% fee (4 bps) for most pools; stableswap pools use 1 bps
        let fee_rate_bps: u32 = 4;
        let fee_tier = FeeTier::Custom(fee_rate_bps);

        Some(PoolRate {
            protocol: Protocol::Curve,
            chain: chain.clone(),
            token0,
            token1,
            pool_type: PoolType::Standard,
            fee_tier,
            fee_rate_bps,
            tvl_usd: tvl,
            volume_24h_usd: volume_24h,
            volume_7d_usd: 0.0, // Volumes API only provides daily snapshot
            fees_24h_usd: fees_24h,
            fees_7d_usd: 0.0,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr,
            pool_address: address.clone(),
            pool_id: Some(address),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn supported_chains() -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
            Chain::Avalanche,
            Chain::Fantom,
        ]
    }

    pub fn get_pool_url(chain: &Chain, pool_address: &str) -> String {
        let slug = chain_to_slug(chain).unwrap_or("ethereum");
        format!("https://curve.fi/#/{}/pools/{}", slug, pool_address)
    }
}

#[async_trait]
impl PoolIndexer for CurveIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Curve
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

fn chain_to_slug(chain: &Chain) -> Option<&'static str> {
    match chain {
        Chain::Ethereum => Some("ethereum"),
        Chain::Arbitrum => Some("arbitrum"),
        Chain::Base => Some("base"),
        Chain::Polygon => Some("polygon"),
        Chain::Optimism => Some("optimism"),
        Chain::Avalanche => Some("avalanche"),
        Chain::Fantom => Some("fantom"),
        _ => None,
    }
}
