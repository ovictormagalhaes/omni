use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Frax Ether (sfrxETH) - Official Frax API Integration
// ============================================================================
// Liquid staking for ETH.
// API: https://api.frax.finance/v2/frxeth/summary/latest
// Supported chains: Ethereum
// ============================================================================

const FRAX_API_URL: &str = "https://api.frax.finance/v2/frxeth/summary/latest";

// ── API response structures ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FraxEthSummary {
    #[serde(default, rename = "sfrxethApr")]
    sfrxeth_apr: Option<f64>,
    #[serde(default, rename = "sfrxethTotalAssets")]
    sfrxeth_total_assets: Option<f64>,
    #[serde(default, rename = "frxethTotalSupply")]
    #[allow(dead_code)]
    frxeth_total_supply: Option<f64>,
    #[serde(default, rename = "sfrxethFrxethPrice")]
    #[allow(dead_code)]
    sfrxeth_frxeth_price: Option<f64>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FraxEthIndexer {
    pub client: reqwest::Client,
}

impl Default for FraxEthIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl FraxEthIndexer {
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

        tracing::info!("[FraxETH] Fetching rates from official Frax API");

        let response = self
            .client
            .get(FRAX_API_URL)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("[FraxETH] API returned status {}", response.status());
            return Ok(vec![]);
        }

        let summary: FraxEthSummary = match response.json().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("[FraxETH] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        let apr = summary.sfrxeth_apr.unwrap_or(0.0);
        let total_assets_eth = summary.sfrxeth_total_assets.unwrap_or(0.0);

        // Approximate TVL in USD (use ETH price ~$2000 as rough estimate;
        // the actual APR is the important metric here)
        let eth_price_approx = 2000.0;
        let tvl_usd = total_assets_eth * eth_price_approx;

        if !(0.0..=1000.0).contains(&apr) {
            tracing::warn!("[FraxETH] Suspicious APR: {:.4}%, skipping", apr);
            return Ok(vec![]);
        }

        let mut rates = Vec::new();

        if tvl_usd > 1000.0 {
            rates.push(ProtocolRate {
                protocol: Protocol::FraxEth,
                chain: Chain::Ethereum,
                asset: Asset::from_symbol("ETH", "FraxETH"),
                action: Action::Supply,
                supply_apy: (apr * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl_usd as u64,
                total_liquidity: tvl_usd as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Staking,
                vault_id: Some("frax-sfrxeth".to_string()),
                vault_name: Some("Frax Ether (sfrxETH)".to_string()),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[FraxETH] Fetched {} rates (APR: {:.2}%)", rates.len(), apr);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.frax.finance/frxeth/mint".to_string()
    }
}

#[async_trait]
impl RateIndexer for FraxEthIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::FraxEth
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
        let indexer = FraxEthIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(
            result.is_ok(),
            "Failed to fetch FraxETH rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        println!("FraxETH: {} rates from official API", rates.len());
        assert!(!rates.is_empty(), "FraxETH should return rates");

        for rate in &rates {
            println!(
                "  {} {}: APY {:.2}%, TVL ${}",
                rate.protocol, rate.asset, rate.supply_apy, rate.total_liquidity
            );
            assert!(rate.supply_apy > 0.0, "APY should be positive");
            assert!(rate.supply_apy < 100.0, "APY should be reasonable");
        }
    }

    #[test]
    fn test_non_ethereum_returns_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let indexer = FraxEthIndexer::new();
        let rates = rt.block_on(indexer.fetch_rates(&Chain::Solana)).unwrap();
        assert!(rates.is_empty());
    }
}
