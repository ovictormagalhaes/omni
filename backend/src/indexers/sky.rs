use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// ============================================================================
// Sky (formerly Maker) - BlockAnalitica API Integration
// ============================================================================
// Sky Savings Rate (sUSDS) + stUSD.
// API: https://info-sky.blockanalitica.com/api/v1/overall/
// Supported chains: Ethereum
// ============================================================================

const SKY_API_URL: &str = "https://info-sky.blockanalitica.com/api/v1/overall/";

// ── API response structures ────────────────────────────────────────────
// Response is an array of objects; first element has the main data

#[derive(Debug, Deserialize)]
struct SkyOverall {
    #[serde(default)]
    sky_savings_rate_apy: Option<String>,
    #[serde(default)]
    sky_savings_rate_tvl: Option<String>,
    #[serde(default)]
    stusds_rate: Option<String>,
    #[serde(default)]
    stusds_tvl: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    total_save: Option<String>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkyIndexer {
    pub client: reqwest::Client,
}

impl SkyIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Ethereum {
            return Ok(vec![]);
        }

        tracing::info!("[Sky] Fetching rates from BlockAnalitica API");

        let response = self.client
            .get(SKY_API_URL)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("[Sky] API returned status {}", response.status());
            return Ok(vec![]);
        }

        // Response is an array; first element has the rates data
        let items: Vec<SkyOverall> = match response.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[Sky] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        let data = match items.first() {
            Some(d) => d,
            None => {
                tracing::warn!("[Sky] Empty response from API");
                return Ok(vec![]);
            }
        };

        let mut rates = Vec::new();

        // Sky Savings Rate (sUSDS) — main savings product
        let ssr_apy = data.sky_savings_rate_apy
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) * 100.0; // API returns decimal (0.0375 = 3.75%)

        let ssr_tvl = data.sky_savings_rate_tvl
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if ssr_tvl > 1000.0 && ssr_apy < 1000.0 {
            rates.push(ProtocolRate {
                protocol: Protocol::Sky,
                chain: Chain::Ethereum,
                asset: Asset::from_symbol("USDS", "Sky"),
                action: Action::Supply,
                supply_apy: (ssr_apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: ssr_tvl as u64,
                total_liquidity: ssr_tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Staking,
                vault_id: Some("sky-ssr-usds".to_string()),
                vault_name: Some("Sky Savings Rate (sUSDS)".to_string()),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        // stUSD rate
        let stusds_rate = data.stusds_rate
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) * 100.0;

        let stusds_tvl = data.stusds_tvl
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if stusds_tvl > 1000.0 && stusds_rate < 1000.0 {
            rates.push(ProtocolRate {
                protocol: Protocol::Sky,
                chain: Chain::Ethereum,
                asset: Asset::from_symbol("SUSDS", "Sky"),
                action: Action::Supply,
                supply_apy: (stusds_rate * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: stusds_tvl as u64,
                total_liquidity: stusds_tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Staking,
                vault_id: Some("sky-stusds".to_string()),
                vault_name: Some("Sky stUSD".to_string()),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[Sky] Fetched {} rates from BlockAnalitica", rates.len());
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.sky.money".to_string()
    }
}

#[async_trait]
impl RateIndexer for SkyIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Sky
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rates() {
        let indexer = SkyIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok(), "Failed to fetch Sky rates: {:?}", result.err());

        let rates = result.unwrap();
        println!("Sky: {} rates from BlockAnalitica API", rates.len());
        assert!(!rates.is_empty(), "Sky should return rates");

        for rate in &rates {
            println!("  {} {}: APY {:.2}%, TVL ${}",
                rate.protocol, rate.asset, rate.supply_apy, rate.total_liquidity);
        }
    }

    #[test]
    fn test_non_ethereum_returns_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let indexer = SkyIndexer::new();
        let rates = rt.block_on(indexer.fetch_rates(&Chain::Solana)).unwrap();
        assert!(rates.is_empty());
    }
}
