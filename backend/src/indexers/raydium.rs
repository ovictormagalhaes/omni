use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::PoolIndexer;
use crate::models::{Asset, Chain, FeeTier, PoolRate, PoolType, Protocol};

#[derive(Clone)]
pub struct RaydiumIndexer {
    client: reqwest::Client,
    api_url: String,
}

// API response structures

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Option<ApiData>,
}

#[derive(Debug, Deserialize)]
struct ApiData {
    data: Vec<PoolInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolInfo {
    #[serde(rename = "type")]
    pool_type: String,
    id: String,
    #[serde(rename = "mintA")]
    mint_a: MintInfo,
    #[serde(rename = "mintB")]
    mint_b: MintInfo,
    #[serde(default)]
    fee_rate: Option<f64>,
    tvl: f64,
    day: Option<PeriodData>,
    week: Option<PeriodData>,
}

#[derive(Debug, Deserialize)]
struct MintInfo {
    symbol: String,
    #[allow(dead_code)]
    address: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeriodData {
    volume: f64,
    volume_fee: f64,
    fee_apr: f64,
    #[serde(default)]
    reward_apr: Vec<f64>,
}

impl RaydiumIndexer {
    pub fn new(api_url: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_url,
        }
    }

    async fn fetch_page(&self, page: u32) -> Result<Vec<PoolInfo>> {
        let url = format!(
            "{}/pools/info/list?poolType=all&poolSortField=liquidity&sortType=desc&pageSize=500&page={}",
            self.api_url, page
        );

        let response: ApiResponse = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .json()
            .await?;

        Ok(response.data.map(|d| d.data).unwrap_or_default())
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Raydium] Fetching pools (2 pages of 500)");

        let (page1, page2) = tokio::join!(self.fetch_page(1), self.fetch_page(2));

        let mut pools = page1.unwrap_or_else(|e| {
            tracing::warn!("[Raydium] Page 1 failed: {}", e);
            vec![]
        });
        pools.extend(page2.unwrap_or_else(|e| {
            tracing::warn!("[Raydium] Page 2 failed: {}", e);
            vec![]
        }));

        if pools.is_empty() {
            tracing::warn!("[Raydium] API returned no data");
            return Ok(vec![]);
        }

        tracing::info!("[Raydium] Fetched {} pools", pools.len());

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter(|p| p.tvl > 0.0)
            .filter_map(|p| self.parse_pool(p))
            .collect();

        tracing::info!("[Raydium] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(&self, pool: PoolInfo) -> Option<PoolRate> {
        let token0 = Asset::from_symbol(&pool.mint_a.symbol, "Raydium");
        let token1 = Asset::from_symbol(&pool.mint_b.symbol, "Raydium");

        let pool_type = match pool.pool_type.as_str() {
            "Concentrated" => PoolType::ConcentratedLiquidity,
            _ => PoolType::Standard,
        };

        // fee_rate is a decimal (e.g., 0.0025 = 0.25% = 25 bps)
        let fee_rate_bps = pool
            .fee_rate
            .map(|r| (r * 10000.0).round() as u32)
            .unwrap_or(30); // default 0.30%

        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        let day = pool.day.as_ref();
        let week = pool.week.as_ref();

        let volume_24h = day.map(|d| d.volume).unwrap_or(0.0);
        let volume_7d = week.map(|w| w.volume).unwrap_or(0.0);
        let fees_24h = day.map(|d| d.volume_fee).unwrap_or(0.0);
        let fees_7d = week.map(|w| w.volume_fee).unwrap_or(0.0);

        // APR from API is already a percentage (e.g., 5.23 = 5.23%)
        let fee_apr_24h = day.map(|d| d.fee_apr).unwrap_or(0.0);
        let fee_apr_7d = week.map(|w| w.fee_apr).unwrap_or(0.0);

        // Sum reward APRs
        let rewards_apr = day.map(|d| d.reward_apr.iter().sum::<f64>()).unwrap_or(0.0);

        Some(PoolRate {
            protocol: Protocol::Raydium,
            chain: Chain::Solana,
            token0,
            token1,
            pool_type,
            fee_tier,
            fee_rate_bps,
            tvl_usd: pool.tvl,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr,
            pool_address: pool.id.clone(),
            pool_id: Some(pool.id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(pool_id: &str) -> String {
        format!(
            "https://raydium.io/liquidity/increase/?mode=add&pool_id={}",
            pool_id
        )
    }
}

#[async_trait]
impl PoolIndexer for RaydiumIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Raydium
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
        Self::get_pool_url(pool.pool_id.as_deref().unwrap_or(&pool.pool_address))
    }
}
