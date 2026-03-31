use anyhow::Result;
use chrono::Utc;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use crate::indexers::defillama_pools::{self, DefiLlamaCache};

// ============================================================================
// Sommelier - DeFiLlama Integration
// ============================================================================
// Automated DeFi vault strategies. DeFiLlama project: "sommelier"
// Supported chains: Ethereum
// ============================================================================

#[derive(Debug, Clone)]
pub struct SommelierIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl SommelierIndexer {
    pub fn new() -> Self {
        Self { client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap_or_default(), defillama_cache: None }
    }

    pub fn with_cache(mut self, cache: DefiLlamaCache) -> Self {
        self.defillama_cache = Some(cache);
        self
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Ethereum { return Ok(vec![]); }

        tracing::info!("[Sommelier] Fetching rates from DeFiLlama");

        let pools = match &self.defillama_cache {
            Some(cache) => cache.get_pools().await?.to_vec(),
            None => defillama_pools::fetch_defillama_pools(&self.client).await?,
        };

        let mut rates = Vec::new();

        for pool in &pools {
            let project = pool.project.as_deref().unwrap_or_default().to_lowercase();
            if project != "sommelier" { continue; }
            let ch = pool.chain.as_deref().unwrap_or_default().to_lowercase();
            if ch != "ethereum" { continue; }

            let symbol_raw = pool.symbol.as_deref().unwrap_or_default();
            let symbol = symbol_raw.split('-').next().unwrap_or(symbol_raw).to_uppercase();
            let asset = Asset::from_symbol(&symbol, "Sommelier");

            let apy = pool.apy_base.unwrap_or(0.0);
            let reward = pool.apy_reward.unwrap_or(0.0);
            let tvl = pool.tvl_usd.unwrap_or(0.0);

            if tvl < 1000.0 || apy > 1000.0 { continue; }

            rates.push(ProtocolRate {
                protocol: Protocol::Sommelier,
                chain: Chain::Ethereum,
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
                vault_name: Some(format!("Sommelier {}", symbol_raw)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[Sommelier] Fetched {} rates", rates.len());
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.sommelier.finance".to_string()
    }
}
