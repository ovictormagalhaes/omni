//! Shared DeFiLlama yields API client with per-cycle cache.
//! Used by: PancakeSwap, Aerodrome, Velodrome, Compound, Venus, Pendle,
//!          SparkLend, Jupiter, Lido, Jito, Ethena, EtherFi (13 indexers).
//! Fetches once per worker cycle, all indexers filter from the cached data.

use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::models::{Chain, PoolType};

#[derive(Debug, Deserialize)]
pub struct DefiLlamaYieldsResponse {
    pub status: Option<String>,
    pub data: Vec<DefiLlamaPool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefiLlamaPool {
    pub chain: Option<String>,
    pub project: Option<String>,
    pub symbol: Option<String>,
    pub pool: Option<String>,
    #[serde(rename = "tvlUsd")]
    pub tvl_usd: Option<f64>,
    #[serde(rename = "apyBase")]
    pub apy_base: Option<f64>,
    #[serde(rename = "apyReward")]
    pub apy_reward: Option<f64>,
    pub apy: Option<f64>,
    #[serde(rename = "volumeUsd1d")]
    pub volume_usd_1d: Option<f64>,
    #[serde(rename = "volumeUsd7d")]
    pub volume_usd_7d: Option<f64>,
    #[serde(rename = "apyBase7d")]
    pub apy_base_7d: Option<f64>,
    #[serde(rename = "poolMeta")]
    pub pool_meta: Option<String>,
    pub stablecoin: Option<bool>,
    #[serde(rename = "ilRisk")]
    pub il_risk: Option<String>,
    pub exposure: Option<String>,
    #[serde(rename = "rewardTokens")]
    pub reward_tokens: Option<Vec<String>>,
}

/// Shared cache for DeFiLlama yields data.
/// Create ONE instance per worker cycle, pass to all indexers via Arc.
#[derive(Clone, Debug)]
pub struct DefiLlamaCache {
    client: reqwest::Client,
    data: Arc<OnceCell<Vec<DefiLlamaPool>>>,
}

impl DefiLlamaCache {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            data: Arc::new(OnceCell::new()),
        }
    }

    /// Get all DeFiLlama pools. Fetches once, returns cached data on subsequent calls.
    pub async fn get_pools(&self) -> Result<&[DefiLlamaPool]> {
        let pools = self
            .data
            .get_or_try_init(|| async {
                tracing::info!("[DeFiLlama] Fetching yields (single shared request)...");
                let response: DefiLlamaYieldsResponse = self
                    .client
                    .get("https://yields.llama.fi/pools")
                    .send()
                    .await?
                    .json()
                    .await?;
                tracing::info!("[DeFiLlama] Cached {} pools", response.data.len());
                Ok::<_, anyhow::Error>(response.data)
            })
            .await?;
        Ok(pools.as_slice())
    }

    /// Filter pools by project name(s). Convenience method.
    pub async fn get_pools_by_project(&self, projects: &[&str]) -> Result<Vec<DefiLlamaPool>> {
        let all = self.get_pools().await?;
        Ok(all
            .iter()
            .filter(|p| {
                p.project
                    .as_deref()
                    .map(|proj| projects.iter().any(|target| proj == *target))
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }
}

/// Legacy: fetch without cache (for standalone use outside worker)
pub async fn fetch_defillama_pools(client: &reqwest::Client) -> Result<Vec<DefiLlamaPool>> {
    let response: DefiLlamaYieldsResponse = client
        .get("https://yields.llama.fi/pools")
        .send()
        .await?
        .json()
        .await?;

    Ok(response.data)
}

/// Map DeFiLlama chain name to our Chain enum
pub fn parse_chain(chain_str: &str) -> Option<Chain> {
    match chain_str {
        "Ethereum" => Some(Chain::Ethereum),
        "BSC" | "Binance" => Some(Chain::BSC),
        "Polygon" => Some(Chain::Polygon),
        "Arbitrum" => Some(Chain::Arbitrum),
        "Optimism" => Some(Chain::Optimism),
        "Base" => Some(Chain::Base),
        "Avalanche" => Some(Chain::Avalanche),
        "Fantom" => Some(Chain::Fantom),
        "Solana" => Some(Chain::Solana),
        "Celo" => Some(Chain::Celo),
        "Blast" => Some(Chain::Blast),
        "Linea" => Some(Chain::Linea),
        "Scroll" => Some(Chain::Scroll),
        "Mantle" => Some(Chain::Mantle),
        "zkSync Era" | "zkSync" => Some(Chain::ZkSync),
        _ => None,
    }
}

/// Parse pool symbol "TOKEN0-TOKEN1" into (token0, token1)
pub fn parse_symbol(symbol: &str) -> (String, String) {
    let parts: Vec<&str> = symbol.splitn(2, '-').collect();
    if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (symbol.to_string(), String::new())
    }
}

/// Determine pool type from symbol or metadata
pub fn infer_pool_type(symbol: &str, project: &str) -> PoolType {
    if project.contains("slipstream") || project.contains("v3") || project.contains("cl") {
        PoolType::ConcentratedLiquidity
    } else if symbol.contains("CL-") || symbol.contains("vAMM") {
        PoolType::ConcentratedLiquidity
    } else {
        PoolType::Standard
    }
}
