use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// ============================================================================
// Lido Finance - Official API
// ============================================================================
// Source: https://eth-api.lido.fi/v1/protocol/steth/apr/sma (APR)
//         https://eth-api.lido.fi/v1/protocol/steth/stats (TVL)
// Supported chains: Ethereum
// ============================================================================

#[derive(Debug, Deserialize)]
struct LidoAprResponse {
    data: LidoAprData,
}

#[derive(Debug, Deserialize)]
struct LidoAprData {
    #[serde(rename = "smaApr")]
    sma_apr: f64,
}

#[derive(Debug, Deserialize)]
struct LidoStatsResponse {
    #[serde(rename = "totalStaked")]
    #[allow(dead_code)]
    total_staked: String,
    #[serde(rename = "marketCap")]
    market_cap: f64,
}

#[derive(Debug, Clone)]
pub struct LidoIndexer {
    client: reqwest::Client,
}

impl LidoIndexer {
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

        tracing::info!("[Lido] Fetching rates from official API");

        // Fetch APR
        let apr_resp = self.client
            .get("https://eth-api.lido.fi/v1/protocol/steth/apr/sma")
            .send()
            .await?;

        if !apr_resp.status().is_success() {
            tracing::warn!("[Lido] APR API returned status: {}", apr_resp.status());
            return Ok(vec![]);
        }

        let apr_data: LidoAprResponse = apr_resp.json().await?;
        let apy = apr_data.data.sma_apr / 100.0; // API returns percentage (e.g. 2.5 = 2.5%)

        // Fetch TVL
        let tvl = match self.client
            .get("https://eth-api.lido.fi/v1/protocol/steth/stats")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let stats: LidoStatsResponse = resp.json().await?;
                stats.market_cap as u64
            }
            _ => 9_000_000_000, // fallback
        };

        let mut rates = Vec::new();

        // stETH
        rates.push(ProtocolRate {
            protocol: Protocol::Lido,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("STETH", "Lido"),
            action: Action::Supply,
            supply_apy: apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: Some(0.10), // Lido takes 10% fee
            active: true,
            collateral_enabled: false,
            collateral_ltv: 0.0,
            available_liquidity: tvl,
            total_liquidity: tvl,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("steth".to_string()),
            vault_name: Some("Lido Staked ETH".to_string()),
            underlying_asset: None,
            timestamp: Utc::now(),
        });

        // wstETH (same APY)
        rates.push(ProtocolRate {
            protocol: Protocol::Lido,
            chain: Chain::Ethereum,
            asset: Asset::from_symbol("WSTETH", "Lido"),
            action: Action::Supply,
            supply_apy: apy,
            borrow_apr: 0.0,
            rewards: 0.0,
            performance_fee: Some(0.10),
            active: true,
            collateral_enabled: false,
            collateral_ltv: 0.0,
            available_liquidity: tvl,
            total_liquidity: tvl,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("wsteth".to_string()),
            vault_name: Some("Wrapped Lido Staked ETH".to_string()),
            underlying_asset: None,
            timestamp: Utc::now(),
        });

        tracing::info!("[Lido] APY: {:.2}%, TVL: ${}", apy * 100.0, tvl);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://lido.fi/".to_string()
    }
}

#[async_trait]
impl RateIndexer for LidoIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Lido
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::Solana]
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

    #[test]
    fn test_parse_apr_response() {
        let json = r#"{"data": {"smaApr": 3.45}}"#;
        let resp: LidoAprResponse = serde_json::from_str(json).unwrap();
        assert!((resp.data.sma_apr - 3.45).abs() < 0.001);
    }

    #[test]
    fn test_parse_stats_response() {
        let json = r#"{"totalStaked": "9876543", "marketCap": 15000000000.0}"#;
        let resp: LidoStatsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.market_cap as u64, 15_000_000_000);
    }

    #[tokio::test]
    async fn test_unsupported_chain_returns_empty() {
        let indexer = LidoIndexer::new();
        let rates = indexer.fetch_rates(&Chain::Solana).await.unwrap();
        assert!(rates.is_empty(), "Lido indexer should return empty for Solana (only Ethereum impl)");
    }

    #[test]
    fn test_indexer_metadata() {
        let indexer = LidoIndexer::new();
        assert_eq!(indexer.get_protocol_url(), "https://lido.fi/");
    }
}
