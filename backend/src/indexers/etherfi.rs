use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::indexers::defillama_pools::DefiLlamaCache;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// EtherFi - DeFiLlama Integration
// ============================================================================
// Liquid restaking protocol. eETH (rebasing) + weETH (non-rebasing wrapper).
// DeFiLlama project names: "ether.fi-stake" (staking), "ether.fi-liquid" (liquid vaults)
// Chain: Ethereum (primary)
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
    apy: Option<f64>,
    #[serde(rename = "apyBase")]
    apy_base: Option<f64>,
    #[serde(rename = "apyReward")]
    apy_reward: Option<f64>,
    #[serde(rename = "totalSupplyUsd")]
    #[allow(dead_code)]
    total_supply_usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EtherFiIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl EtherFiIndexer {
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
        if *chain != Chain::Ethereum {
            return Ok(vec![]);
        }

        tracing::info!("[EtherFi] Fetching rates from DeFiLlama");

        let etherfi_pools: Vec<DefiLlamaPool> = if let Some(ref cache) = self.defillama_cache {
            cache
                .get_pools()
                .await?
                .iter()
                .filter(|p| {
                    let proj = p.project.as_deref().unwrap_or("").to_lowercase();
                    (proj == "ether.fi-stake" || proj == "ether.fi-liquid")
                        && p.chain
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case("ethereum"))
                })
                .map(|p| DefiLlamaPool {
                    pool: p.pool.clone().unwrap_or_default(),
                    chain: p.chain.clone().unwrap_or_default(),
                    project: p.project.clone().unwrap_or_default(),
                    symbol: p.symbol.clone().unwrap_or_default(),
                    tvl_usd: p.tvl_usd,
                    apy: p.apy,
                    apy_base: p.apy_base,
                    apy_reward: p.apy_reward,
                    total_supply_usd: None,
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
                    "[EtherFi] DeFiLlama API returned status: {}",
                    response.status()
                );
                return Ok(vec![]);
            }

            let pools_response: DefiLlamaPoolResponse = response.json().await?;

            pools_response
                .data
                .into_iter()
                .filter(|p| {
                    let proj = p.project.to_lowercase();
                    (proj == "ether.fi-stake" || proj == "ether.fi-liquid")
                        && p.chain.to_lowercase() == "ethereum"
                })
                .collect()
        };

        let mut rates = Vec::new();

        for pool in etherfi_pools {
            let symbol = normalize_symbol(&pool.symbol);
            let asset = Asset::from_symbol(&symbol, "EtherFi");

            let supply_apy = pool.apy_base.unwrap_or(pool.apy.unwrap_or(0.0));
            let rewards = pool.apy_reward.unwrap_or(0.0);
            let tvl = pool.tvl_usd.unwrap_or(0.0);

            if tvl < 1000.0 {
                continue;
            }
            if supply_apy > 100.0 {
                continue;
            }

            let op_type = if pool.project.to_lowercase().contains("liquid") {
                OperationType::Vault
            } else {
                OperationType::Staking
            };

            rates.push(ProtocolRate {
                protocol: Protocol::EtherFi,
                chain: Chain::Ethereum,
                asset,
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: (rewards * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 100.0,
                ltv: 0.0,
                operation_type: op_type,
                vault_id: Some(pool.pool.clone()),
                vault_name: Some(pool.symbol.clone()),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[EtherFi] Fetched {} rates", rates.len());
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.ether.fi/liquid".to_string()
    }
}

#[async_trait]
impl RateIndexer for EtherFiIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::EtherFi
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
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
    s.split(|c: char| c == '-' || c == '/' || c == ' ')
        .next()
        .unwrap_or(&s)
        .to_string()
}
