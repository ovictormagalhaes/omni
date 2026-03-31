use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// SparkLend - BlockAnalitica API Integration
// ============================================================================
// SparkLend is an Aave v3 fork on Ethereum.
// API: https://spark-api.blockanalitica.com/v1/ethereum/markets/
// Supported chain: Ethereum
// ============================================================================

const SPARK_API_URL: &str = "https://spark-api.blockanalitica.com/v1/ethereum/markets/";

const SUPPORTED_ASSETS: &[&str] = &[
    "USDC", "USDT", "DAI", "USDS", "SUSDS", "WETH", "ETH", "WSTETH", "STETH", "WBTC", "RETH",
    "SDAI", "CBBTC", "GNO", "CBETH",
];

// ── API response structures ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SparkMarket {
    symbol: String,
    underlying_address: Option<String>,
    total_supply_usd: Option<f64>,
    total_borrow_usd: Option<f64>,
    tvl_usd: Option<f64>,
    supply_apy: Option<String>,
    borrow_variable_apy: Option<String>,
    utilization_rate: Option<f64>,
    usage_as_collateral_enabled: Option<bool>,
    borrowing_enabled: Option<bool>,
    is_frozen: Option<bool>,
    ltv: Option<String>,
    #[allow(dead_code)]
    liquidation_threshold: Option<String>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SparkLendIndexer {
    pub client: reqwest::Client,
}

impl Default for SparkLendIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl SparkLendIndexer {
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
            tracing::debug!("[SparkLend] Unsupported chain {:?}, skipping", chain);
            return Ok(vec![]);
        }

        tracing::info!("[SparkLend] Fetching rates from BlockAnalitica API");

        let response = self
            .client
            .get(SPARK_API_URL)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!("[SparkLend] API returned status {}", response.status());
            return Ok(vec![]);
        }

        let markets: Vec<SparkMarket> = match response.json().await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("[SparkLend] Failed to parse API response: {}", e);
                return Ok(vec![]);
            }
        };

        tracing::debug!("[SparkLend] Found {} markets from API", markets.len());

        let mut rates = Vec::new();

        for market in markets {
            let symbol = normalize_symbol(&market.symbol);

            if !SUPPORTED_ASSETS.contains(&symbol.as_str()) {
                continue;
            }

            let asset = Asset::from_symbol(&symbol, "SparkLend");

            // API returns APY as string decimal (e.g. "0.012118" = 1.2118%)
            let supply_apy = market
                .supply_apy
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
                * 100.0;

            let borrow_apr = market
                .borrow_variable_apy
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
                * 100.0;

            let total_supply = market.total_supply_usd.unwrap_or(0.0);
            let total_borrow = market.total_borrow_usd.unwrap_or(0.0);
            let tvl = market.tvl_usd.unwrap_or(0.0);

            // utilization_rate comes as decimal (e.g. 0.738 = 73.8%)
            let utilization_rate = market.utilization_rate.unwrap_or(0.0) * 100.0;
            let available_liquidity = (total_supply - total_borrow).max(0.0);

            // LTV comes as string decimal (e.g. "0.8500" = 85%)
            let ltv = market
                .ltv
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let collateral_enabled = market.usage_as_collateral_enabled.unwrap_or(false);
            let is_frozen = market.is_frozen.unwrap_or(false);
            let borrowing_enabled = market.borrowing_enabled.unwrap_or(false);

            if tvl < 1000.0 {
                continue;
            }
            if supply_apy > 1000.0 || borrow_apr > 1000.0 {
                continue;
            }

            // Supply rate
            rates.push(ProtocolRate {
                protocol: Protocol::SparkLend,
                chain: Chain::Ethereum,
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0,
                performance_fee: None,
                active: !is_frozen,
                collateral_enabled,
                collateral_ltv: ltv,
                available_liquidity: available_liquidity as u64,
                total_liquidity: total_supply as u64,
                utilization_rate,
                ltv,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: market.underlying_address.clone(),
                timestamp: Utc::now(),
            });

            // Borrow rate
            if borrowing_enabled {
                rates.push(ProtocolRate {
                    protocol: Protocol::SparkLend,
                    chain: Chain::Ethereum,
                    asset,
                    action: Action::Borrow,
                    supply_apy: (supply_apy * 100.0).round() / 100.0,
                    borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                    rewards: 0.0,
                    performance_fee: None,
                    active: !is_frozen && borrowing_enabled,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity: available_liquidity as u64,
                    total_liquidity: total_supply as u64,
                    utilization_rate,
                    ltv,
                    operation_type: OperationType::Lending,
                    vault_id: None,
                    vault_name: None,
                    underlying_asset: market.underlying_address.clone(),
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!("[SparkLend] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self, _chain: &Chain, _underlying_asset: Option<&str>) -> String {
        "https://app.spark.fi/markets/".to_string()
    }
}

#[async_trait]
impl RateIndexer for SparkLendIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::SparkLend
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain, rate.underlying_asset.as_deref())
    }
}

fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.to_uppercase();
    let base = s.split(['-', '/', ' ']).next().unwrap_or(&s);
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_symbol() {
        assert_eq!(normalize_symbol("WETH"), "WETH");
        assert_eq!(normalize_symbol("USDC-V2"), "USDC");
        assert_eq!(normalize_symbol("weth"), "WETH");
        assert_eq!(normalize_symbol("DAI/USDC"), "DAI");
    }

    #[test]
    fn test_get_protocol_url() {
        let indexer = SparkLendIndexer::new();
        let url = indexer.get_protocol_url(&Chain::Ethereum, None);
        assert!(url.contains("spark.fi"));
    }

    #[test]
    fn test_non_ethereum_returns_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let indexer = SparkLendIndexer::new();

        let rates = rt.block_on(indexer.fetch_rates(&Chain::Solana)).unwrap();
        assert!(rates.is_empty());

        let rates = rt.block_on(indexer.fetch_rates(&Chain::Arbitrum)).unwrap();
        assert!(rates.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_rates_ethereum() {
        let indexer = SparkLendIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(
            result.is_ok(),
            "Failed to fetch SparkLend rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        println!(
            "SparkLend Ethereum: {} rates from BlockAnalitica API",
            rates.len()
        );
        assert!(!rates.is_empty(), "SparkLend should return rates");

        let supply_rates: Vec<_> = rates
            .iter()
            .filter(|r| r.action == Action::Supply)
            .collect();
        assert!(!supply_rates.is_empty(), "Should have supply rates");

        let borrow_rates: Vec<_> = rates
            .iter()
            .filter(|r| r.action == Action::Borrow)
            .collect();
        assert!(!borrow_rates.is_empty(), "Should have borrow rates");

        for rate in rates.iter().take(5) {
            println!(
                "  {} {:?} {}: APY {:.2}%, Borrow {:.2}%, Liquidity ${}, Util {:.1}%",
                rate.protocol,
                rate.action,
                rate.asset,
                rate.supply_apy,
                rate.borrow_apr,
                rate.available_liquidity,
                rate.utilization_rate
            );
            assert!(rate.supply_apy < 100.0, "Supply APY should be reasonable");
            assert!(rate.borrow_apr < 100.0, "Borrow APR should be reasonable");
        }
    }
}
