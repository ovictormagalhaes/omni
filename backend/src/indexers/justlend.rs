use anyhow::Result;
use serde::Deserialize;

use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct JustLendIndexer {
    client: reqwest::Client,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JustLendMarket {
    #[serde(rename = "contractAddress")]
    contract_address: String,
    symbol: String,
    #[serde(rename = "supplyApy")]
    supply_apy: Option<f64>,
    #[serde(rename = "borrowApy")]
    borrow_apy: Option<f64>,
    #[serde(rename = "totalSupply")]
    total_supply: Option<f64>,
    #[serde(rename = "totalBorrows")]
    total_borrows: Option<f64>,
    #[serde(rename = "underlyingSymbol")]
    underlying_symbol: Option<String>,
}

impl JustLendIndexer {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        // JustLend only operates on Tron
        if *chain != Chain::Tron {
            return Ok(Vec::new());
        }

        tracing::info!("Fetching JustLend rates from Tron network");
        
        // JustLend API endpoint
        let url = "https://api.just.network/justlend/markets";
        
        let response = self.client
            .get(url)
            .header("Accept", "application/json")
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::error!("JustLend API returned status {}: blocking access. Protocol may require authentication or be unavailable.", response.status());
            return Ok(Vec::new());
        }

        let markets: Vec<JustLendMarket> = response.json().await?;
        
        let mut rates = Vec::new();

        for market in markets {
            let underlying = market.underlying_symbol
                .unwrap_or_else(|| market.symbol.replace("j", ""));
            
            let asset = Self::normalize_asset(&underlying);
            if asset.is_none() {
                continue;
            }

            let supply_apy = market.supply_apy.unwrap_or(0.0);
            let borrow_apy = market.borrow_apy.unwrap_or(0.0);
            let total_supply = market.total_supply.unwrap_or(0.0);
            let total_borrows = market.total_borrows.unwrap_or(0.0);
            
            let available_liquidity = (total_supply - total_borrows) as u64;
            let total_liquidity = total_supply as u64;
            let utilization_rate = if total_supply > 0.0 {
                (total_borrows / total_supply) * 100.0
            } else {
                0.0
            };

            // Supply rate
            if supply_apy > 0.0 {
                rates.push(ProtocolRate {
                    protocol: Protocol::JustLend,
                    chain: Chain::Tron,
                    asset: asset.clone().unwrap(),
                    action: Action::Supply,
                    supply_apy,
                    borrow_apr: 0.0,
                    rewards: 0.0,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: true,
                    collateral_ltv: 0.75,
                    available_liquidity,
                    total_liquidity,
                    utilization_rate,
                    ltv: 0.0,
                    operation_type: OperationType::Lending,
                    vault_id: Some(market.contract_address.clone()),
                    vault_name: Some(format!("JustLend {}", underlying)),
                    underlying_asset: Some(underlying.clone()),
                    timestamp: Utc::now(),
                });
            }

            // Borrow rate
            if borrow_apy > 0.0 {
                rates.push(ProtocolRate {
                    protocol: Protocol::JustLend,
                    chain: Chain::Tron,
                    asset: asset.unwrap(),
                    action: Action::Borrow,
                    supply_apy: 0.0,
                    borrow_apr: borrow_apy,
                    rewards: 0.0,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity,
                    total_liquidity,
                    utilization_rate,
                    ltv: 0.0,
                    operation_type: OperationType::Lending,
                    vault_id: Some(market.contract_address.clone()),
                    vault_name: Some(format!("JustLend {}", underlying)),
                    underlying_asset: Some(underlying),
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!("JustLend: fetched {} rates", rates.len());
        Ok(rates)
    }

    fn normalize_asset(symbol: &str) -> Option<Asset> {
        let symbol = symbol.to_uppercase();
        match symbol.as_str() {
            "USDT" => Some(Asset::from_symbol("USDT", "JustLend")),
            "USDC" => Some(Asset::from_symbol("USDC", "JustLend")),
            "USDD" => Some(Asset::from_symbol("USDD", "JustLend")),
            "TRX" => Some(Asset::from_symbol("TRX", "JustLend")),
            "BTC" | "WBTC" => Some(Asset::from_symbol("WBTC", "JustLend")),
            "ETH" | "WETH" => Some(Asset::from_symbol("WETH", "JustLend")),
            _ => {
                tracing::debug!("JustLend: Unknown asset {}", symbol);
                None
            }
        }
    }

    pub fn get_protocol_url(&self) -> String {
        "https://justlend.org/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_rates_tron() {
        let indexer = JustLendIndexer::new(None);
        let result = indexer.fetch_rates(&Chain::Tron).await;
        
        // May fail due to network issues
        match result {
            Ok(rates) => {
                println!("JustLend Tron: {} rates", rates.len());
                for rate in rates.iter().take(3) {
                    println!("  {} {} {}: APY {:.2}%", 
                        rate.protocol, rate.chain, rate.asset, 
                        if rate.action == Action::Supply { rate.supply_apy } else { rate.borrow_apr });
                }
            }
            Err(e) => {
                println!("JustLend test failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_rates_non_tron() {
        let indexer = JustLendIndexer::new(None);
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        
        let rates = result.unwrap();
        assert_eq!(rates.len(), 0); // Should return empty for non-Tron chains
    }
}