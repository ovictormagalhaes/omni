use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// Marinade Finance - Solana Liquid Staking
// Provides mSOL liquid staking token

#[derive(Debug, Deserialize)]
struct MarinadeApyResponse {
    value: f64,
    #[allow(dead_code)]
    end_time: String,
    #[allow(dead_code)]
    end_price: f64,
    #[allow(dead_code)]
    start_time: String,
    #[allow(dead_code)]
    start_price: f64,
}

#[derive(Debug, Clone)]
pub struct MarinadeIndexer {
    client: reqwest::Client,
}

impl MarinadeIndexer {
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
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Marinade Finance staking APY from official API");
        
        // Marinade official APY endpoint
        let url = "https://api.marinade.finance/msol/apy/7d";
        
        let response = self.client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("Marinade API returned status: {}", response.status());
            return Ok(Vec::new());
        }

        let apy_response: MarinadeApyResponse = response.json().await?;
        let supply_apy = apy_response.value;
        
        let mut rates = Vec::new();

        rates.push(ProtocolRate {
            protocol: Protocol::Marinade,
            chain: Chain::Solana,
            asset: Asset::from_symbol("MSOL", "Marinade"),
            action: Action::Supply,
            supply_apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: None,
            active: true,
            collateral_enabled: false,  // Staking doesn't provide collateral
            collateral_ltv: 0.0,
            available_liquidity: 420_000_000,  // Approximate TVL
            total_liquidity: 420_000_000,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("msol".to_string()),
            vault_name: Some("Marinade Staked SOL".to_string()),
            underlying_asset: Some("So11111111111111111111111111111111111111112".to_string()),
            timestamp: Utc::now(),
        });

        tracing::info!("Marinade: fetched {} rates with APY {:.2}%", rates.len(), supply_apy);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://marinade.finance/".to_string()
    }
}

#[async_trait]
impl RateIndexer for MarinadeIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Marinade
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
        let indexer = MarinadeIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].asset, Asset::from_symbol("MSOL", "Marinade"));
        assert_eq!(rates[0].operation_type, OperationType::Staking);
    }

    #[tokio::test]
    async fn test_fetch_rates_wrong_chain() {
        let indexer = MarinadeIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
