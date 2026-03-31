use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use super::RateIndexer;
use crate::indexers::defillama_pools::{self, DefiLlamaCache};
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Convex Finance - DeFiLlama Integration
// ============================================================================
// Yield booster for Curve LP positions. Convex API only provides partial data
// (APY without TVL/metadata). DeFiLlama provides full data including TVL.
// DeFiLlama project: "convex-finance"
// Supported chains: Ethereum, Arbitrum
// ============================================================================

#[derive(Debug, Clone)]
pub struct ConvexIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl Default for ConvexIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConvexIndexer {
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
            Chain::Ethereum => "Ethereum",
            Chain::Arbitrum => "Arbitrum",
            _ => return Ok(vec![]),
        };

        tracing::info!("[Convex] Fetching rates for {:?} from DeFiLlama", chain);

        let pools = match &self.defillama_cache {
            Some(cache) => cache.get_pools().await?.to_vec(),
            None => defillama_pools::fetch_defillama_pools(&self.client).await?,
        };

        let mut rates = Vec::new();

        for pool in &pools {
            let project = pool.project.as_deref().unwrap_or_default();
            if !project.eq_ignore_ascii_case("convex-finance") {
                continue;
            }
            let ch = pool.chain.as_deref().unwrap_or_default();
            if !ch.eq_ignore_ascii_case(chain_name) {
                continue;
            }

            let symbol_raw = pool.symbol.as_deref().unwrap_or_default();
            let symbol = symbol_raw
                .split('-')
                .next()
                .unwrap_or(symbol_raw)
                .to_uppercase();
            let asset = Asset::from_symbol(&symbol, "Convex");

            let apy = pool.apy_base.unwrap_or(0.0);
            let reward = pool.apy_reward.unwrap_or(0.0);
            let tvl = pool.tvl_usd.unwrap_or(0.0);

            if tvl < 1000.0 || apy > 1000.0 {
                continue;
            }

            rates.push(ProtocolRate {
                protocol: Protocol::Convex,
                chain: chain.clone(),
                asset,
                action: Action::Supply,
                supply_apy: (apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: (reward * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Vault,
                vault_id: pool.pool.clone(),
                vault_name: Some(format!("Convex {}", symbol_raw)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[Convex] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://www.convexfinance.com/stake".to_string()
    }
}

#[async_trait]
impl RateIndexer for ConvexIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Convex
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::Arbitrum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}
