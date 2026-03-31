use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::indexers::defillama_pools::DefiLlamaCache;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Radiant Capital - DeFiLlama Integration
// ============================================================================
// Cross-chain lending protocol (Aave V2 fork). Uses DeFiLlama yields API.
// DeFiLlama project name: "radiant-v2"
// Supported chains: Arbitrum, BSC, Base, Ethereum
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
pub struct RadiantIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl RadiantIndexer {
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
            Chain::Arbitrum => "arbitrum",
            Chain::BSC => "bsc",
            Chain::Base => "base",
            Chain::Ethereum => "ethereum",
            _ => {
                tracing::debug!("Radiant doesn't support chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[Radiant] Fetching rates for {:?} from DeFiLlama", chain);

        let radiant_pools: Vec<DefiLlamaPool> = if let Some(ref cache) = self.defillama_cache {
            cache
                .get_pools()
                .await?
                .iter()
                .filter(|p| {
                    p.project
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case("radiant-v2"))
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
                    "[Radiant] DeFiLlama API returned status: {}",
                    response.status()
                );
                return Ok(vec![]);
            }

            let pools_response: DefiLlamaPoolResponse = response.json().await?;

            pools_response
                .data
                .into_iter()
                .filter(|p| {
                    p.project.to_lowercase() == "radiant-v2" && p.chain.to_lowercase() == chain_name
                })
                .collect()
        };

        tracing::debug!(
            "[Radiant] Found {} pools on {:?}",
            radiant_pools.len(),
            chain
        );

        let mut rates = Vec::new();

        for pool in radiant_pools {
            let symbol = normalize_symbol(&pool.symbol);
            let asset = Asset::from_symbol(&symbol, "Radiant");

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

            rates.push(ProtocolRate {
                protocol: Protocol::Radiant,
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

            if borrow_apr > 0.0 || total_borrow > 0.0 {
                rates.push(ProtocolRate {
                    protocol: Protocol::Radiant,
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

        tracing::info!("[Radiant] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain) -> String {
        let chain_slug = match chain {
            Chain::Arbitrum => "42161",
            Chain::BSC => "56",
            Chain::Base => "8453",
            Chain::Ethereum => "1",
            _ => "42161",
        };
        format!(
            "https://app.radiant.capital/#/markets?chainId={}",
            chain_slug
        )
    }
}

#[async_trait]
impl RateIndexer for RadiantIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Radiant
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Arbitrum, Chain::BSC, Chain::Base, Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain)
    }
}

fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.to_uppercase();
    s.split(|c: char| c == '-' || c == '/' || c == ' ')
        .next()
        .unwrap_or(&s)
        .to_string()
}
