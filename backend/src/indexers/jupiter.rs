use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};

// Jupiter is primarily a DEX aggregator and Perps platform
// They don't have a traditional lending protocol like Aave/Kamino
// Keeping this indexer minimal with just JupSOL staking vault
// NOTE: Jupiter Perps is NOT lending - it's perpetual futures trading

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
pub struct JupiterIndexer {
    client: reqwest::Client,
}

impl JupiterIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        // Jupiter operates on Solana
        if *chain != Chain::Solana {
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Jupiter staking APY from DeFi Llama");
        
        // DeFi Llama API for Jupiter pool
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
        
        // Find Jupiter pool
        let jupiter_pool = pools_response.data.iter()
            .find(|p| p.symbol.to_uppercase().contains("JUPSOL") || p.symbol.to_uppercase().contains("JUPITER"));
        
        if let Some(pool) = jupiter_pool {
            let mut rates = Vec::new();
            
            rates.push(ProtocolRate {
                protocol: Protocol::Jupiter,
                chain: Chain::Solana,
                asset: Asset::from_symbol("JUPSOL", "Jupiter"),
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
                vault_id: Some("jupsol-staking".to_string()),
                vault_name: Some("JupSOL Staking".to_string()),
                underlying_asset: Some("So11111111111111111111111111111111111111112".to_string()),
                timestamp: Utc::now(),
            });
            
            tracing::info!("Jupiter: fetched {} rates with APY {:.2}%", rates.len(), pool.apy);
            Ok(rates)
        } else {
            tracing::warn!("Jupiter pool not found in DeFi Llama response");
            Ok(Vec::new())
        }
    }

    pub fn get_protocol_url(&self) -> String {
        "https://station.jup.ag/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rates_solana() {
        let indexer = JupiterIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 1); // Only JupSOL staking
        assert_eq!(rates[0].asset, Asset::from_symbol("JUPSOL", "Jupiter"));
        
        println!("Jupiter Solana: {} product (staking only, not lending)", rates.len());
        for rate in rates.iter() {
            println!("  {} {} {}: Base APY {:.2}% + Rewards {:.2}%", 
                rate.protocol, rate.chain, rate.asset, 
                rate.supply_apy, rate.rewards);
        }
    }

    #[tokio::test]
    async fn test_fetch_rates_non_solana() {
        let indexer = JupiterIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 0); // Should return empty for non-Solana chains
    }
}
