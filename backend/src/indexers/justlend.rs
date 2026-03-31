use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::indexers::defillama_pools::DefiLlamaCache;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// JustLend - DeFiLlama Integration
// ============================================================================
// Lending protocol on Tron. Uses DeFiLlama yields API because the official
// JustLend API (api.just.network) is behind Cloudflare bot protection.
// DeFiLlama project name: "justlend"
// Supported chains: Tron
// ============================================================================

const DEFILLAMA_POOLS_URL: &str = "https://yields.llama.fi/pools";

#[derive(Debug, Deserialize)]
struct DefiLlamaPoolResponse {
    data: Vec<DefiLlamaPool>,
}

#[derive(Debug, Deserialize)]
struct DefiLlamaPool {
    pool: String,
    chain: String,
    project: String,
    symbol: String,
    #[serde(rename = "tvlUsd")]
    tvl_usd: Option<f64>,
    #[serde(rename = "apyBase")]
    apy_base: Option<f64>,
    #[serde(rename = "apyReward")]
    apy_reward: Option<f64>,
    #[serde(rename = "apyBaseBorrow")]
    apy_base_borrow: Option<f64>,
    #[serde(rename = "apyRewardBorrow")]
    apy_reward_borrow: Option<f64>,
    #[serde(rename = "totalSupplyUsd")]
    total_supply_usd: Option<f64>,
    #[serde(rename = "totalBorrowUsd")]
    total_borrow_usd: Option<f64>,
    #[serde(rename = "ltv")]
    ltv: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct JustLendIndexer {
    client: reqwest::Client,
    #[allow(dead_code)]
    api_key: Option<String>,
    defillama_cache: Option<DefiLlamaCache>,
}

impl JustLendIndexer {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_key,
            defillama_cache: None,
        }
    }

    pub fn with_cache(mut self, cache: DefiLlamaCache) -> Self {
        self.defillama_cache = Some(cache);
        self
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Tron {
            return Ok(Vec::new());
        }

        tracing::info!("[JustLend] Fetching rates from DeFiLlama");

        let justlend_pools: Vec<DefiLlamaPool> = if let Some(ref cache) = self.defillama_cache {
            cache
                .get_pools()
                .await?
                .iter()
                .filter(|p| {
                    p.project
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case("justlend"))
                        && p.chain
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case("tron"))
                })
                .map(|p| DefiLlamaPool {
                    pool: p.pool.clone().unwrap_or_default(),
                    chain: p.chain.clone().unwrap_or_default(),
                    project: p.project.clone().unwrap_or_default(),
                    symbol: p.symbol.clone().unwrap_or_default(),
                    tvl_usd: p.tvl_usd,
                    apy_base: p.apy_base,
                    apy_reward: p.apy_reward,
                    apy_base_borrow: None,
                    apy_reward_borrow: None,
                    total_supply_usd: None,
                    total_borrow_usd: None,
                    ltv: None,
                })
                .collect()
        } else {
            let response = self
                .client
                .get(DEFILLAMA_POOLS_URL)
                .header("Accept", "application/json")
                .send()
                .await?;

            if !response.status().is_success() {
                tracing::warn!(
                    "[JustLend] DeFiLlama API returned status: {}",
                    response.status()
                );
                return Ok(vec![]);
            }

            let pools_response: DefiLlamaPoolResponse = response.json().await?;

            pools_response
                .data
                .into_iter()
                .filter(|p| {
                    p.project.to_lowercase() == "justlend" && p.chain.to_lowercase() == "tron"
                })
                .collect()
        };

        tracing::debug!(
            "[JustLend] Found {} pools on DeFiLlama",
            justlend_pools.len()
        );

        let mut rates = Vec::new();

        for pool in justlend_pools {
            let symbol = normalize_symbol(&pool.symbol);
            let asset = Asset::from_symbol(&symbol, "JustLend");

            let supply_apy = pool.apy_base.unwrap_or(0.0);
            let supply_reward = pool.apy_reward.unwrap_or(0.0);
            let borrow_apr = pool.apy_base_borrow.unwrap_or(0.0).abs();
            let borrow_reward = pool.apy_reward_borrow.unwrap_or(0.0);

            let total_supply = pool.total_supply_usd.unwrap_or(0.0);
            let total_borrow = pool.total_borrow_usd.unwrap_or(0.0);
            let tvl = pool.tvl_usd.unwrap_or(0.0);

            let utilization_rate = if total_supply > 0.0 {
                (total_borrow / total_supply * 100.0).min(100.0)
            } else {
                0.0
            };

            let available_liquidity = (total_supply - total_borrow).max(0.0);
            let ltv = pool.ltv.unwrap_or(0.0) / 100.0;

            if tvl < 1000.0 {
                continue;
            }
            if supply_apy > 1000.0 || borrow_apr > 1000.0 {
                continue;
            }

            // Supply rate
            rates.push(ProtocolRate {
                protocol: Protocol::JustLend,
                chain: Chain::Tron,
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy,
                borrow_apr: 0.0,
                rewards: supply_reward,
                performance_fee: None,
                active: true,
                collateral_enabled: true,
                collateral_ltv: ltv,
                available_liquidity: available_liquidity as u64,
                total_liquidity: tvl as u64,
                utilization_rate,
                ltv,
                operation_type: OperationType::Lending,
                vault_id: Some(pool.pool.clone()),
                vault_name: Some(format!("JustLend {}", symbol)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });

            // Borrow rate (if data available)
            if borrow_apr > 0.0 || total_borrow > 0.0 {
                rates.push(ProtocolRate {
                    protocol: Protocol::JustLend,
                    chain: Chain::Tron,
                    asset,
                    action: Action::Borrow,
                    supply_apy: 0.0,
                    borrow_apr,
                    rewards: borrow_reward,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity: available_liquidity as u64,
                    total_liquidity: tvl as u64,
                    utilization_rate,
                    ltv,
                    operation_type: OperationType::Lending,
                    vault_id: Some(pool.pool.clone()),
                    vault_name: Some(format!("JustLend {}", symbol)),
                    underlying_asset: None,
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!("[JustLend] Fetched {} rates from DeFiLlama", rates.len());
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://justlend.org/".to_string()
    }
}

/// Normalize DeFiLlama pool symbols (e.g., "WBTC" -> first token only)
fn normalize_symbol(symbol: &str) -> String {
    // DeFiLlama symbols can be like "USDT", "WBTC", etc.
    // Take the first token if there are multiple (e.g., "USDC-USDT" -> "USDC")
    symbol
        .split(['-', '/', ' '])
        .next()
        .unwrap_or(symbol)
        .to_uppercase()
}

#[async_trait]
impl RateIndexer for JustLendIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::JustLend
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Tron]
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
    async fn test_fetch_rates_tron() {
        let indexer = JustLendIndexer::new(None);
        let result = indexer.fetch_rates(&Chain::Tron).await;

        match result {
            Ok(rates) => {
                println!("JustLend (DeFiLlama): {} rates", rates.len());
                for rate in rates.iter().take(3) {
                    println!(
                        "  {} {} {}: APY {:.2}%",
                        rate.protocol,
                        rate.chain,
                        rate.asset,
                        if rate.action == Action::Supply {
                            rate.supply_apy
                        } else {
                            rate.borrow_apr
                        }
                    );
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
        assert_eq!(rates.len(), 0);
    }

    #[test]
    fn test_normalize_symbol() {
        assert_eq!(normalize_symbol("USDT"), "USDT");
        assert_eq!(normalize_symbol("USDC-USDT"), "USDC");
        assert_eq!(normalize_symbol("wbtc"), "WBTC");
    }

    #[test]
    fn test_justlend_protocol_url() {
        let indexer = JustLendIndexer::new(None);
        assert_eq!(indexer.get_protocol_url(), "https://justlend.org/");
    }
}
