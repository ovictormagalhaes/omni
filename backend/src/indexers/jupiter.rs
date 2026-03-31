use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Jupiter - Official API Integration (JupSOL Liquid Staking)
// ============================================================================
// Jupiter is a Solana DEX aggregator that also offers JupSOL liquid staking.
// API: https://worker.jup.ag/lst-apys (public, no auth)
// Returns a map of LST mint addresses → APY (decimal).
// Supported chain: Solana
// ============================================================================

const JUPITER_LST_APYS_URL: &str = "https://worker.jup.ag/lst-apys";

// JupSOL mint address on Solana
const JUPSOL_MINT: &str = "jupSoLaHXQiZZTSfEWMTRRgpnyFm8f6sZdosWBjx93v";

// ── API response structure ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JupiterLstApysResponse {
    apys: HashMap<String, f64>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct JupiterIndexer {
    client: reqwest::Client,
}

impl Default for JupiterIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl JupiterIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Solana {
            return Ok(vec![]);
        }

        tracing::info!("[Jupiter] Fetching JupSOL APY from official API");

        let response = self
            .client
            .get(JUPITER_LST_APYS_URL)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("[Jupiter] API returned status {}", response.status());
            return Ok(vec![]);
        }

        let data: JupiterLstApysResponse = match response.json().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("[Jupiter] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        tracing::debug!("[Jupiter] Got APYs for {} LSTs", data.apys.len());

        let mut rates = Vec::new();

        // JupSOL APY
        if let Some(&apy_decimal) = data.apys.get(JUPSOL_MINT) {
            // API returns decimal (e.g. 0.0619 = 6.19%)
            let apy = apy_decimal * 100.0;

            if !(0.0..=100.0).contains(&apy) {
                tracing::warn!("[Jupiter] Suspicious JupSOL APY: {:.4}%, skipping", apy);
                return Ok(vec![]);
            }

            rates.push(ProtocolRate {
                protocol: Protocol::Jupiter,
                chain: Chain::Solana,
                asset: Asset::from_symbol("JUPSOL", "Jupiter"),
                action: Action::Supply,
                supply_apy: (apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: 0,
                total_liquidity: 0,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Staking,
                vault_id: Some(JUPSOL_MINT.to_string()),
                vault_name: Some("JupSOL Staking".to_string()),
                underlying_asset: Some("So11111111111111111111111111111111111111112".to_string()),
                timestamp: Utc::now(),
            });

            tracing::info!("[Jupiter] JupSOL APY: {:.2}%", apy);
        } else {
            tracing::warn!("[Jupiter] JupSOL mint not found in API response");
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://station.jup.ag/".to_string()
    }
}

#[async_trait]
impl RateIndexer for JupiterIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Jupiter
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Solana]
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
    async fn test_fetch_rates_solana() {
        let indexer = JupiterIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(
            result.is_ok(),
            "Failed to fetch Jupiter rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        println!("Jupiter: {} rates from official API", rates.len());
        assert!(!rates.is_empty(), "Jupiter should return JupSOL rate");

        let rate = &rates[0];
        assert_eq!(rate.protocol, Protocol::Jupiter);
        assert_eq!(rate.chain, Chain::Solana);
        assert_eq!(rate.operation_type, OperationType::Staking);
        assert!(rate.supply_apy > 0.0, "JupSOL should have positive APY");
        assert!(rate.supply_apy < 50.0, "JupSOL APY should be reasonable");

        println!("  JupSOL: APY {:.2}%", rate.supply_apy);
    }

    #[tokio::test]
    async fn test_fetch_rates_non_solana() {
        let indexer = JupiterIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
