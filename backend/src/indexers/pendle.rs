use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// ============================================================================
// Pendle Finance - Official API V2
// ============================================================================
// Source: https://api-v2.pendle.finance/core/v1/{chainId}/markets
// Supported chains: Ethereum (1), Arbitrum (42161), BSC (56), Base (8453)
// ============================================================================

#[derive(Debug, Deserialize)]
struct PendleResponse {
    results: Vec<PendleMarket>,
}

#[derive(Debug, Deserialize)]
struct PendleMarket {
    address: String,
    symbol: String,
    #[serde(rename = "aggregatedApy")]
    aggregated_apy: Option<f64>,
    liquidity: Option<PendleLiquidity>,
    #[serde(rename = "underlyingAsset")]
    underlying_asset: Option<PendleAsset>,
    pt: Option<PendleAsset>,
}

#[derive(Debug, Deserialize)]
struct PendleLiquidity {
    usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PendleAsset {
    symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendleIndexer {
    client: reqwest::Client,
}

impl PendleIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let chain_id = match chain {
            Chain::Ethereum => "1",
            Chain::Arbitrum => "42161",
            Chain::BSC => "56",
            Chain::Base => "8453",
            _ => return Ok(vec![]),
        };

        tracing::info!("[Pendle] Fetching markets for chain {} from official API", chain_id);

        let url = format!(
            "https://api-v2.pendle.finance/core/v1/{}/markets?limit=100&order_by=liquidity:1",
            chain_id
        );

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            tracing::warn!("[Pendle] API returned status: {}", resp.status());
            return Ok(vec![]);
        }

        let data: PendleResponse = resp.json().await?;
        let mut rates = Vec::new();

        for market in &data.results {
            let tvl = market.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            if tvl < 10000.0 {
                continue;
            }

            let apy = market.aggregated_apy.unwrap_or(0.0);
            if apy <= 0.0 || apy > 10.0 {
                continue; // Skip 0% or >1000% APY
            }

            // Extract underlying asset symbol
            let symbol = market.underlying_asset.as_ref()
                .and_then(|a| a.symbol.as_ref())
                .or_else(|| market.pt.as_ref().and_then(|p| p.symbol.as_ref()))
                .map(|s| {
                    // Clean PT-xxx-DATE -> xxx
                    let cleaned = s.trim_start_matches("PT-");
                    cleaned.split('-').next().unwrap_or(cleaned).to_uppercase()
                })
                .unwrap_or_else(|| "UNKNOWN".to_string());

            let asset = Asset::from_symbol(&symbol, "Pendle");

            rates.push(ProtocolRate {
                protocol: Protocol::Pendle,
                chain: chain.clone(),
                asset,
                action: Action::Supply,
                supply_apy: (apy * 100.0 * 100.0).round() / 100.0, // apy is decimal
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Vault,
                vault_id: Some(market.address.clone()),
                vault_name: Some(format!("Pendle {}", market.symbol)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[Pendle] Fetched {} markets for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain, vault_id: Option<&str>) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::BSC => "bnbchain",
            Chain::Base => "base",
            _ => return "https://app.pendle.finance/trade/markets".to_string(),
        };

        match vault_id {
            Some(address) => format!(
                "https://app.pendle.finance/trade/markets/{}/swap?view=pt&chain={}",
                address, chain_slug
            ),
            None => "https://app.pendle.finance/trade/markets".to_string(),
        }
    }
}

#[async_trait]
impl RateIndexer for PendleIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Pendle
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::Arbitrum, Chain::BSC, Chain::Base]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain, rate.vault_id.as_deref())
    }
}
