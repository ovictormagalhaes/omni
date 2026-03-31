use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// Rocket Pool - Decentralized Ethereum Staking
// Provides rETH liquid staking token

#[derive(Debug, Deserialize)]
struct RocketPoolResponse {
    #[serde(rename = "rethAPR")]
    reth_apr: String,
}

#[derive(Debug, Clone)]
pub struct RocketPoolIndexer {
    client: reqwest::Client,
}

impl RocketPoolIndexer {
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
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Rocket Pool staking APR from official API");
        
        // Rocket Pool official API endpoint
        let url = "https://rocketpool.net/api/mainnet/payload";
        
        let response = self.client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("Rocket Pool API returned status: {}", response.status());
            return Ok(Vec::new());
        }

        let rp_response: RocketPoolResponse = response.json().await?;
        
        // Parse APR from string (API returns very long decimal strings)
        // The API returns APR as a percentage value (e.g., "2.41780..." = 2.42%)
        let supply_apy: f64 = rp_response.reth_apr
            .parse()
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to parse Rocket Pool APR: {}", rp_response.reth_apr);
                0.0
            });
        
        let mut rates = Vec::new();

        rates.push(ProtocolRate {
            protocol: Protocol::RocketPool,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("RETH", "Rocket Pool"),
            action: Action::Supply,
            supply_apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: None,
            active: true,
            collateral_enabled: false,  // Staking doesn't provide collateral
            collateral_ltv: 0.0,
            available_liquidity: 2_800_000_000,  // Approximate TVL
            total_liquidity: 2_800_000_000,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("reth".to_string()),
            vault_name: Some("Rocket Pool ETH".to_string()),
            underlying_asset: Some("0x0000000000000000000000000000000000000000".to_string()),
            timestamp: Utc::now(),
        });

        tracing::info!("Rocket Pool: fetched {} rates with APY {:.4}%", rates.len(), supply_apy);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://rocketpool.net/".to_string()
    }
}

#[async_trait]
impl RateIndexer for RocketPoolIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::RocketPool
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
    async fn test_fetch_rates_ethereum() {
        let indexer = RocketPoolIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].asset, Asset::from_symbol("RETH", "Rocket Pool"));
        assert_eq!(rates[0].operation_type, OperationType::Staking);
    }

    #[tokio::test]
    async fn test_fetch_rates_wrong_chain() {
        let indexer = RocketPoolIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
