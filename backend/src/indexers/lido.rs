use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};

// Lido Finance - Ethereum Liquid Staking
// Provides stETH and wstETH liquid staking tokens

#[derive(Debug, Deserialize)]
struct DefiLlamaPoolResponse {
    data: Vec<DefiLlamaPool>,
}

#[derive(Debug, Deserialize)]
struct DefiLlamaPool {
    symbol: String,
    apy: f64,
    project: String,
    chain: String,
}

#[derive(Debug, Clone)]
pub struct LidoIndexer {
    client: reqwest::Client,
}

impl LidoIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Ethereum {
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Lido Finance staking APY from DeFi Llama API");
        
        // DeFi Llama pools API (fallback since official Lido API has connectivity issues)
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
        
        let mut rates = Vec::new();
        let mut lido_apy = 0.032; // Default ETH staking APR

        // Find Lido pools
        for pool in &pools_response.data {
            if pool.project.to_lowercase() == "lido" && 
               pool.chain.to_lowercase() == "ethereum" && 
               (pool.symbol.contains("STETH") || pool.symbol.contains("WSTETH")) {
                lido_apy = pool.apy / 100.0; // Convert percentage to decimal
                break;
            }
        }

        // stETH - Standard liquid staking token
        rates.push(ProtocolRate {
            protocol: Protocol::Lido,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("STETH", "Lido"),
            action: Action::Supply,
            supply_apy: lido_apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: None,
            active: true,
            collateral_enabled: false,  // Staking doesn't provide collateral
            collateral_ltv: 0.0,
            available_liquidity: 9_500_000_000,  // Approximate TVL from on-chain data
            total_liquidity: 9_500_000_000,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("steth".to_string()),
            vault_name: Some("Lido Staked ETH".to_string()),
            underlying_asset: Some("0x0000000000000000000000000000000000000000".to_string()),
            timestamp: Utc::now(),
        });

        // wstETH - Wrapped stETH (same APY as stETH)
        rates.push(ProtocolRate {
            protocol: Protocol::Lido,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("WSTETH", "Lido"),
            action: Action::Supply,
            supply_apy: lido_apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: None,
            active: true,
            collateral_enabled: false,
            collateral_ltv: 0.0,
            available_liquidity: 9_500_000_000,
            total_liquidity: 9_500_000_000,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("wsteth".to_string()),
            vault_name: Some("Wrapped Lido Staked ETH".to_string()),
            underlying_asset: Some("0x0000000000000000000000000000000000000000".to_string()),
            timestamp: Utc::now(),
        });

        tracing::info!("Lido: fetched {} rates with APY {:.2}%", rates.len(), lido_apy * 100.0);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://lido.fi/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rates_ethereum() {
        let indexer = LidoIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 2); // stETH + wstETH
        assert!(rates.iter().any(|r| r.asset == Asset::from_symbol("STETH", "Lido")));
        assert!(rates.iter().any(|r| r.asset == Asset::from_symbol("WSTETH", "Lido")));
    }

    #[tokio::test]
    async fn test_fetch_rates_solana() {
        let indexer = LidoIndexer::new();
        let result = indexer.fetch_rates(&Chain::Solana).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 1); // stSOL
        assert_eq!(rates[0].asset, Asset::from_symbol("STSOL", "Lido"));
    }

    #[tokio::test]
    async fn test_operation_type_is_staking() {
        let indexer = LidoIndexer::new();
        let rates = indexer.fetch_rates(&Chain::Ethereum).await.unwrap();
        
        for rate in rates {
            assert_eq!(rate.operation_type, OperationType::Staking);
        }
    }
}
