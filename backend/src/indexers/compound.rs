use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use super::RateIndexer;
use crate::indexers::defillama_pools::DefiLlamaCache;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Compound V3 (Comet) - DeFiLlama Integration
// ============================================================================
// Multi-chain lending protocol. Compound V3 has no public REST API.
// Uses DeFiLlama yields API (shared cache).
// DeFiLlama project name: "compound-v3"
// Supported chains: Ethereum, Arbitrum, Base, Polygon, Optimism
// ============================================================================

#[derive(Debug, Clone)]
pub struct CompoundIndexer {
    pub client: reqwest::Client,
    pub defillama_cache: Option<DefiLlamaCache>,
}

impl Default for CompoundIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl CompoundIndexer {
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
            Chain::Base => "Base",
            Chain::Polygon => "Polygon",
            Chain::Optimism => "Optimism",
            _ => {
                tracing::debug!("[Compound V3] Unsupported chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!(
            "[Compound V3] Fetching rates for {:?} from DeFiLlama",
            chain
        );

        let pools = match &self.defillama_cache {
            Some(cache) => cache.get_pools().await?.to_vec(),
            None => crate::indexers::defillama_pools::fetch_defillama_pools(&self.client).await?,
        };

        let mut rates = Vec::new();

        for pool in &pools {
            let project = pool.project.as_deref().unwrap_or_default();
            if !project.eq_ignore_ascii_case("compound-v3") {
                continue;
            }
            let ch = pool.chain.as_deref().unwrap_or_default();
            if !ch.eq_ignore_ascii_case(chain_name) {
                continue;
            }

            let symbol_raw = pool.symbol.as_deref().unwrap_or_default();
            let symbol = normalize_symbol(symbol_raw);
            let asset = Asset::from_symbol(&symbol, "Compound");

            let supply_apy = pool.apy_base.unwrap_or(0.0);
            let supply_reward = pool.apy_reward.unwrap_or(0.0);
            let tvl = pool.tvl_usd.unwrap_or(0.0);

            if tvl < 1000.0 {
                continue;
            }
            if supply_apy > 1000.0 {
                continue;
            }

            let pool_id = pool.pool.clone();

            // Supply rate
            rates.push(ProtocolRate {
                protocol: Protocol::Compound,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: (supply_reward * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: true,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Lending,
                vault_id: pool_id.clone(),
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });

            // Borrow rate (DeFiLlama doesn't provide borrow data for compound-v3 in yields)
        }

        tracing::info!(
            "[Compound V3] Fetched {} rates for {:?}",
            rates.len(),
            chain
        );
        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "",
            Chain::Arbitrum => "?market=arbitrum",
            Chain::Base => "?market=base",
            Chain::Polygon => "?market=polygon",
            Chain::Optimism => "?market=optimism",
            _ => "",
        };
        format!("https://app.compound.finance/markets{}", chain_slug)
    }
}

#[async_trait]
impl RateIndexer for CompoundIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Compound
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
        ]
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
    s.split(['-', '/', ' ']).next().unwrap_or(&s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_symbol() {
        assert_eq!(normalize_symbol("USDC"), "USDC");
        assert_eq!(normalize_symbol("WETH"), "WETH");
        assert_eq!(normalize_symbol("usdc"), "USDC");
        assert_eq!(normalize_symbol("USDC-V2"), "USDC");
    }

    #[tokio::test]
    async fn test_fetch_rates_ethereum() {
        let indexer = CompoundIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(
            result.is_ok(),
            "Failed to fetch Compound rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        println!("Compound Ethereum: {} rates from DeFiLlama", rates.len());
        assert!(!rates.is_empty(), "Compound should return rates");

        for rate in rates.iter().take(5) {
            println!(
                "  {} {:?} {}: APY {:.2}%, Rewards {:.2}%",
                rate.protocol, rate.action, rate.asset, rate.supply_apy, rate.rewards
            );
        }
    }

    #[test]
    fn test_unsupported_chain() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let indexer = CompoundIndexer::new();
        let rates = rt.block_on(indexer.fetch_rates(&Chain::Solana)).unwrap();
        assert!(rates.is_empty());
    }
}
