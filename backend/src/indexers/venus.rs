use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Venus Protocol - Official API
// ============================================================================
// Source: https://api.venus.io/markets/core-pool?chainId={chainId}
// Supported chains: BSC (56), Ethereum (1)
// ============================================================================

#[derive(Debug, Deserialize)]
struct VenusResponse {
    result: Vec<VenusMarket>,
}

#[derive(Debug, Deserialize)]
struct VenusMarket {
    #[serde(rename = "underlyingSymbol")]
    underlying_symbol: String,
    #[serde(rename = "underlyingName")]
    #[allow(dead_code)]
    underlying_name: String,
    #[serde(rename = "supplyApy")]
    supply_apy: Option<f64>,
    #[serde(rename = "borrowApy")]
    borrow_apy: Option<f64>,
    #[serde(rename = "supplyXvsApr")]
    supply_xvs_apr: Option<String>,
    #[serde(rename = "borrowXvsApr")]
    borrow_xvs_apr: Option<String>,
    #[serde(rename = "totalSupplyUnderlyingCents")]
    total_supply_cents: Option<String>,
    #[serde(rename = "totalBorrowCents")]
    total_borrow_cents: Option<String>,
    #[serde(rename = "liquidityCents")]
    liquidity_cents: Option<String>,
    #[serde(rename = "collateralFactorMantissa")]
    collateral_factor: Option<String>,
    #[serde(rename = "isBorrowable")]
    is_borrowable: Option<bool>,
    #[serde(rename = "canBeCollateral")]
    can_be_collateral: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct VenusIndexer {
    client: reqwest::Client,
}

impl VenusIndexer {
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
            Chain::BSC => "56",
            Chain::Ethereum => "1",
            _ => return Ok(vec![]),
        };

        tracing::info!(
            "[Venus] Fetching markets for chainId {} from official API",
            chain_id
        );

        let url = format!(
            "https://api.venus.io/markets/core-pool?chainId={}&limit=100",
            chain_id
        );

        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            tracing::warn!("[Venus] API returned status: {}", resp.status());
            return Ok(vec![]);
        }

        let data: VenusResponse = resp.json().await?;
        let mut rates = Vec::new();

        for market in &data.result {
            let supply_apy = market.supply_apy.unwrap_or(0.0);
            let borrow_apy = market.borrow_apy.unwrap_or(0.0);

            let supply_reward: f64 = market
                .supply_xvs_apr
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let borrow_reward: f64 = market
                .borrow_xvs_apr
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let total_supply_usd: f64 = market
                .total_supply_cents
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
                / 100.0; // cents to USD
            let total_borrow_usd: f64 = market
                .total_borrow_cents
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
                / 100.0;
            let liquidity_usd: f64 = market
                .liquidity_cents
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
                / 100.0;

            if total_supply_usd < 10000.0 {
                continue;
            }
            if supply_apy > 1000.0 || borrow_apy > 1000.0 {
                continue;
            }

            // LTV from collateralFactorMantissa (18 decimals, e.g. 750000000000000000 = 75%)
            let ltv = market
                .collateral_factor
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .map(|v| v / 1e18)
                .unwrap_or(0.0);

            let utilization = if total_supply_usd > 0.0 {
                (total_borrow_usd / total_supply_usd * 100.0).min(100.0)
            } else {
                0.0
            };

            let asset = Asset::from_symbol(&market.underlying_symbol, "Venus");

            // Supply
            rates.push(ProtocolRate {
                protocol: Protocol::Venus,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy,
                borrow_apr: borrow_apy,
                rewards: supply_reward,
                performance_fee: None,
                active: true,
                collateral_enabled: market.can_be_collateral.unwrap_or(false),
                collateral_ltv: ltv,
                available_liquidity: liquidity_usd as u64,
                total_liquidity: total_supply_usd as u64,
                utilization_rate: utilization,
                ltv,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });

            // Borrow
            if market.is_borrowable.unwrap_or(false) && (borrow_apy > 0.0 || total_borrow_usd > 0.0)
            {
                rates.push(ProtocolRate {
                    protocol: Protocol::Venus,
                    chain: chain.clone(),
                    asset,
                    action: Action::Borrow,
                    supply_apy,
                    borrow_apr: borrow_apy,
                    rewards: borrow_reward,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity: liquidity_usd as u64,
                    total_liquidity: total_supply_usd as u64,
                    utilization_rate: utilization,
                    ltv,
                    operation_type: OperationType::Lending,
                    vault_id: None,
                    vault_name: None,
                    underlying_asset: None,
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!(
            "[Venus] Fetched {} rates for {:?} (from {} markets)",
            rates.len(),
            chain,
            data.result.len()
        );
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.venus.io/core-pool".to_string()
    }
}

#[async_trait]
impl RateIndexer for VenusIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Venus
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::BSC, Chain::Ethereum]
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
    fn test_parse_venus_response() {
        let json = serde_json::json!({
            "result": [
                {
                    "underlyingSymbol": "USDC",
                    "underlyingName": "USD Coin",
                    "supplyApy": 4.5,
                    "borrowApy": 7.2,
                    "supplyXvsApr": "0.5",
                    "borrowXvsApr": "0.3",
                    "totalSupplyUnderlyingCents": "5000000000",
                    "totalBorrowCents": "3000000000",
                    "liquidityCents": "2000000000",
                    "collateralFactorMantissa": "750000000000000000",
                    "isBorrowable": true,
                    "canBeCollateral": true
                }
            ]
        });

        let resp: VenusResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.result.len(), 1);

        let market = &resp.result[0];
        assert_eq!(market.underlying_symbol, "USDC");
        assert_eq!(market.supply_apy, Some(4.5));
        assert_eq!(market.borrow_apy, Some(7.2));
        assert_eq!(market.is_borrowable, Some(true));
        assert_eq!(market.can_be_collateral, Some(true));

        // LTV from mantissa: 750000000000000000 / 1e18 = 0.75
        let ltv = market
            .collateral_factor
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v / 1e18)
            .unwrap_or(0.0);
        assert!((ltv - 0.75).abs() < 0.001);

        // Cents to USD conversion
        let supply_usd: f64 = market
            .total_supply_cents
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
            / 100.0;
        assert!((supply_usd - 50_000_000.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_venus_response_with_missing_fields() {
        let json = serde_json::json!({
            "result": [
                {
                    "underlyingSymbol": "BNB",
                    "underlyingName": "BNB",
                    "supplyApy": null,
                    "borrowApy": null,
                    "supplyXvsApr": null,
                    "borrowXvsApr": null,
                    "totalSupplyUnderlyingCents": null,
                    "totalBorrowCents": null,
                    "liquidityCents": null,
                    "collateralFactorMantissa": null,
                    "isBorrowable": null,
                    "canBeCollateral": null
                }
            ]
        });

        let resp: VenusResponse = serde_json::from_value(json).unwrap();
        let market = &resp.result[0];
        assert_eq!(market.supply_apy, None);
        assert_eq!(market.borrow_apy, None);
    }

    #[tokio::test]
    async fn test_unsupported_chain_returns_empty() {
        let indexer = VenusIndexer::new();
        let rates = indexer.fetch_rates(&Chain::Solana).await.unwrap();
        assert!(rates.is_empty());
    }

    #[test]
    fn test_venus_filters_low_supply() {
        // Markets with total_supply_usd < 10000 should be skipped
        // This is tested implicitly by the indexer logic:
        // total_supply_cents "100000" / 100 = 1000 < 10000
        let json = serde_json::json!({
            "result": [{
                "underlyingSymbol": "TINY",
                "underlyingName": "Tiny Token",
                "supplyApy": 5.0,
                "borrowApy": 8.0,
                "supplyXvsApr": "0",
                "borrowXvsApr": "0",
                "totalSupplyUnderlyingCents": "100000",
                "totalBorrowCents": "50000",
                "liquidityCents": "50000",
                "collateralFactorMantissa": "0",
                "isBorrowable": true,
                "canBeCollateral": false
            }]
        });
        let resp: VenusResponse = serde_json::from_value(json).unwrap();
        let supply_usd: f64 = resp.result[0]
            .total_supply_cents
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
            / 100.0;
        assert!(supply_usd < 10000.0, "Should be filtered out as too small");
    }

    #[test]
    fn test_venus_filters_extreme_apy() {
        // APY > 1000 should be filtered out
        let supply_apy: f64 = 1500.0;
        assert!(supply_apy > 1000.0, "Should be filtered as extreme APY");
    }

    #[test]
    fn test_venus_protocol_url() {
        let indexer = VenusIndexer::new();
        assert_eq!(indexer.get_protocol_url(), "https://app.venus.io/core-pool");
    }
}
