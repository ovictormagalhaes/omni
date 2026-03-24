use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};

// Jito - Solana MEV Liquid Staking
// JitoSOL with MEV rewards

#[derive(Debug, Deserialize)]
struct DefiLlamaPoolResponse {
    data: Vec<DefiLlamaPool>,
}

#[derive(Debug, Deserialize)]
struct DefiLlamaPool {
    symbol: String,
    apy: f64,
    #[serde(rename = "tvlUsd")]
    tvl_usd: f64,
}

#[derive(Debug, Clone)]
pub struct JitoIndexer {
    client: reqwest::Client,
}

impl JitoIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Solana {
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Jito staking APY from DeFi Llama");
        
        // DeFi Llama API for Jito pool
        let url = "https://yields.llama.fi/pools";
        
        let response = self.client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("DeFi Llama API returned status: {}", response.status());
            return Ok(Vec::new());
        }

        let pools_response: DefiLlamaPoolResponse = response.json().await?;
        
        // Find Jito pool
        let jito_pool = pools_response.data.iter()
            .find(|p| p.symbol.to_uppercase().contains("JITOSOL") || p.symbol.to_uppercase().contains("JITO"));
        
        if let Some(pool) = jito_pool {
            let mut rates = Vec::new();
            
            rates.push(ProtocolRate {
                protocol: Protocol::Jito,
                chain: Chain::Solana,
                asset: Asset::from_symbol("JITOSOL", "Jito"),
                action: Action::Supply,
                supply_apy: pool.apy,
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,  // Staking doesn't provide collateral
                collateral_ltv: 0.0,
                available_liquidity: pool.tvl_usd as u64,
                total_liquidity: pool.tvl_usd as u64,
                utilization_rate: 100.0,
                ltv: 0.0,
                operation_type: OperationType::Staking,
                vault_id: Some("jitosol".to_string()),
                vault_name: Some("Jito Staked SOL".to_string()),
                underlying_asset: Some("So11111111111111111111111111111111111111112".to_string()),
                timestamp: Utc::now(),
            });
            
            tracing::info!("Jito: fetched {} rates with APY {:.2}%", rates.len(), pool.apy);
            Ok(rates)
        } else {
            tracing::warn!("Jito pool not found in DeFi Llama response");
            Ok(Vec::new())
        }
    }

    pub fn get_protocol_url(&self) -> String {
        "https://www.jito.network/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rates_solana() {
        let indexer = JitoIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].asset, Asset::from_symbol("JITOSOL", "Jito"));
        assert_eq!(rates[0].operation_type, OperationType::Staking);
    }

    #[tokio::test]
    async fn test_mev_rewards_included() {
        let indexer = JitoIndexer::new();
        let rates = indexer.fetch_rates(&Chain::Solana).await.unwrap();
        
        assert!(rates[0].rewards > 0.0, "Jito should have MEV rewards");
    }
}
