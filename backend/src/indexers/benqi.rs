use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::indexers::defillama_pools::DefiLlamaCache;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Benqi - DeFiLlama Integration
// ============================================================================
// Lending protocol on Avalanche. Uses DeFiLlama yields API.
// DeFiLlama project name: "benqi-lending"
// Supported chains: Avalanche
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
pub struct BenqiIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl Default for BenqiIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl BenqiIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            defillama_cache: None,
        }
    }

    pub fn with_cache(mut self, cache: DefiLlamaCache) -> Self {
        self.defillama_cache = Some(cache);
        self
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let chain_name = match chain {
            Chain::Avalanche => "avalanche",
            _ => {
                tracing::debug!("Benqi doesn't support chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[Benqi] Fetching rates for {:?} from DeFiLlama", chain);

        let benqi_pools: Vec<DefiLlamaPool> = if let Some(ref cache) = self.defillama_cache {
            cache
                .get_pools()
                .await?
                .iter()
                .filter(|p| {
                    p.project
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case("benqi-lending"))
                        && p.chain
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case(chain_name))
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
                    "[Benqi] DeFiLlama API returned status: {}",
                    response.status()
                );
                return Ok(vec![]);
            }

            let pools_response: DefiLlamaPoolResponse = response.json().await?;

            pools_response
                .data
                .into_iter()
                .filter(|p| {
                    p.project.to_lowercase() == "benqi-lending"
                        && p.chain.to_lowercase() == chain_name
                })
                .collect()
        };

        tracing::debug!("[Benqi] Found {} pools on {:?}", benqi_pools.len(), chain);

        let mut rates = Vec::new();

        for pool in benqi_pools {
            let symbol = normalize_symbol(&pool.symbol);
            let asset = Asset::from_symbol(&symbol, "Benqi");

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
                protocol: Protocol::Benqi,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: (supply_reward * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: ltv > 0.0,
                collateral_ltv: ltv,
                available_liquidity: available_liquidity as u64,
                total_liquidity: total_supply as u64,
                utilization_rate,
                ltv,
                operation_type: OperationType::Lending,
                vault_id: Some(pool.pool.clone()),
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });

            // Borrow rate
            if borrow_apr > 0.0 || total_borrow > 0.0 {
                rates.push(ProtocolRate {
                    protocol: Protocol::Benqi,
                    chain: chain.clone(),
                    asset,
                    action: Action::Borrow,
                    supply_apy: (supply_apy * 100.0).round() / 100.0,
                    borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                    rewards: (borrow_reward.abs() * 100.0).round() / 100.0,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity: available_liquidity as u64,
                    total_liquidity: total_supply as u64,
                    utilization_rate,
                    ltv,
                    operation_type: OperationType::Lending,
                    vault_id: Some(pool.pool.clone()),
                    vault_name: None,
                    underlying_asset: None,
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!("[Benqi] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.benqi.fi/lending".to_string()
    }
}

#[async_trait]
impl RateIndexer for BenqiIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Benqi
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Avalanche]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}

fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.to_uppercase();
    // Benqi symbols may include prefixes like "qi" or suffixes
    let cleaned = s.replace("QI", "");
    cleaned
        .split(['-', '/', ' '])
        .next()
        .unwrap_or(&s)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_symbol_basic() {
        assert_eq!(normalize_symbol("USDC"), "USDC");
        assert_eq!(normalize_symbol("AVAX"), "AVAX");
        assert_eq!(normalize_symbol("usdc"), "USDC");
    }

    #[test]
    fn test_normalize_symbol_strips_qi_prefix() {
        assert_eq!(normalize_symbol("qiUSDC"), "USDC");
        assert_eq!(normalize_symbol("qiAVAX"), "AVAX");
    }

    #[test]
    fn test_normalize_symbol_with_separator() {
        assert_eq!(normalize_symbol("USDC-V2"), "USDC");
        assert_eq!(normalize_symbol("AVAX/USD"), "AVAX");
    }

    #[test]
    fn test_parse_defillama_pool() {
        let json = serde_json::json!({
            "pool": "pool-123",
            "chain": "Avalanche",
            "project": "benqi-lending",
            "symbol": "USDC",
            "tvlUsd": 50000000.0,
            "apyBase": 3.5,
            "apyReward": 0.5,
            "apyBaseBorrow": 5.2,
            "apyRewardBorrow": 0.1,
            "totalSupplyUsd": 60000000.0,
            "totalBorrowUsd": 40000000.0,
            "ltv": 75.0
        });
        let pool: DefiLlamaPool = serde_json::from_value(json).unwrap();
        assert_eq!(pool.project, "benqi-lending");
        assert_eq!(pool.chain, "Avalanche");
        assert!((pool.apy_base.unwrap() - 3.5).abs() < 0.001);
        assert!((pool.tvl_usd.unwrap() - 50_000_000.0).abs() < 0.01);
        // LTV: 75.0 / 100.0 = 0.75
        let ltv = pool.ltv.unwrap_or(0.0) / 100.0;
        assert!((ltv - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_parse_defillama_pool_missing_fields() {
        let json = serde_json::json!({
            "pool": "pool-456",
            "chain": "Avalanche",
            "project": "benqi-lending",
            "symbol": "AVAX"
        });
        let pool: DefiLlamaPool = serde_json::from_value(json).unwrap();
        assert!(pool.tvl_usd.is_none());
        assert!(pool.apy_base.is_none());
        assert!(pool.ltv.is_none());
    }

    #[tokio::test]
    async fn test_unsupported_chain_returns_empty() {
        let indexer = BenqiIndexer::new();
        let rates = indexer.fetch_rates(&Chain::Ethereum).await.unwrap();
        assert!(rates.is_empty());
    }

    #[test]
    fn test_benqi_protocol_url() {
        let indexer = BenqiIndexer::new();
        assert_eq!(indexer.get_protocol_url(), "https://app.benqi.fi/lending");
    }

    #[test]
    fn test_utilization_rate_capped_at_100() {
        let total_supply = 100.0_f64;
        let total_borrow = 150.0_f64; // more borrow than supply (edge case)
        let utilization = (total_borrow / total_supply * 100.0).min(100.0);
        assert!((utilization - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_available_liquidity_never_negative() {
        let total_supply = 100.0_f64;
        let total_borrow = 150.0_f64;
        let available = (total_supply - total_borrow).max(0.0);
        assert_eq!(available, 0.0);
    }
}
