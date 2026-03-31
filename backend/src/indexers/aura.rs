use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Aura Finance - Official GraphQL API Integration
// ============================================================================
// Yield booster for Balancer LP positions.
// API: https://data.aura.finance/graphql (POST, GraphQL)
// Supported chains: Ethereum, Arbitrum, Base, Optimism, Polygon
// ============================================================================

const AURA_GRAPHQL_URL: &str = "https://data.aura.finance/graphql";

const CHAIN_MAP: &[(Chain, u64)] = &[
    (Chain::Ethereum, 1),
    (Chain::Arbitrum, 42161),
    (Chain::Base, 8453),
    (Chain::Optimism, 10),
    (Chain::Polygon, 137),
];

// ── GraphQL response structures ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<PoolsData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct PoolsData {
    pools: Vec<AuraPool>,
}

#[derive(Debug, Deserialize)]
struct AuraPool {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tvl: Option<f64>,
    #[serde(default, rename = "isShutdown")]
    is_shutdown: Option<bool>,
    #[serde(default)]
    tokens: Option<Vec<AuraToken>>,
    #[serde(default)]
    aprs: Option<AuraAprs>,
}

#[derive(Debug, Deserialize)]
struct AuraToken {
    #[serde(default)]
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuraAprs {
    #[serde(default)]
    total: Option<f64>,
    #[serde(default)]
    breakdown: Option<Vec<AprBreakdown>>,
}

#[derive(Debug, Deserialize)]
struct AprBreakdown {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    value: Option<f64>,
}

// ── Indexer implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AuraIndexer {
    pub client: reqwest::Client,
}

impl Default for AuraIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl AuraIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let chain_id = match CHAIN_MAP.iter().find(|(c, _)| c == chain) {
            Some((_, id)) => *id,
            None => return Ok(vec![]),
        };

        tracing::info!(
            "[Aura] Fetching rates for {:?} from official GraphQL API",
            chain
        );

        let query = json!({
            "query": format!(
                r#"{{ pools(chainId: {}) {{ id name address tvl isShutdown tokens {{ symbol }} aprs {{ total breakdown {{ id value }} }} }} }}"#,
                chain_id
            )
        });

        let response = self
            .client
            .post(AURA_GRAPHQL_URL)
            .json(&query)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!(
                "[Aura] API returned status {} for {:?}",
                response.status(),
                chain
            );
            return Ok(vec![]);
        }

        let gql_response: GraphQLResponse = match response.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[Aura] Failed to parse GraphQL response for {:?}: {}",
                    chain,
                    e
                );
                return Ok(vec![]);
            }
        };

        if let Some(errors) = &gql_response.errors {
            let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            tracing::error!("[Aura] GraphQL errors for {:?}: {:?}", chain, msgs);
            return Ok(vec![]);
        }

        let pools = gql_response.data.map(|d| d.pools).unwrap_or_default();

        tracing::debug!("[Aura] Found {} pools on {:?}", pools.len(), chain);

        let mut rates = Vec::new();

        for pool in pools {
            if pool.is_shutdown.unwrap_or(false) {
                continue;
            }

            let tvl = pool.tvl.unwrap_or(0.0);
            if tvl < 1000.0 {
                continue;
            }

            let name = pool.name.as_deref().unwrap_or_default();
            let symbol = extract_primary_symbol(name, &pool.tokens);
            let asset = Asset::from_symbol(&symbol, "Aura");

            // APRs from GraphQL: total is the sum, breakdown has individual components
            let aprs = pool.aprs.as_ref();
            let total_apr = aprs.and_then(|a| a.total).unwrap_or(0.0);

            // Separate base (SWAP_FEES) from reward APRs
            let (base_apr, reward_apr) = match aprs.and_then(|a| a.breakdown.as_ref()) {
                Some(breakdown) => {
                    let base: f64 = breakdown
                        .iter()
                        .filter(|b| {
                            b.id.as_deref()
                                .map(|id| id.contains("SWAP_FEES"))
                                .unwrap_or(false)
                        })
                        .filter_map(|b| b.value)
                        .sum();
                    (base, total_apr - base)
                }
                None => (0.0, total_apr),
            };

            if total_apr > 1000.0 {
                continue;
            }

            let pool_id = pool
                .id
                .or(pool.address)
                .unwrap_or_else(|| format!("aura-{}-{}", chain_id, name));

            rates.push(ProtocolRate {
                protocol: Protocol::Aura,
                chain: chain.clone(),
                asset,
                action: Action::Supply,
                supply_apy: (base_apr * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: (reward_apr * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Vault,
                vault_id: Some(pool_id),
                vault_name: Some(format!("Aura {}", name)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!("[Aura] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.aura.finance".to_string()
    }
}

fn extract_primary_symbol(name: &str, tokens: &Option<Vec<AuraToken>>) -> String {
    if let Some(tokens) = tokens {
        if let Some(first) = tokens.first() {
            if let Some(sym) = &first.symbol {
                if !sym.is_empty() {
                    return sym.to_uppercase();
                }
            }
        }
    }
    name.split(['-', '/', ' '])
        .next()
        .unwrap_or(name)
        .to_uppercase()
}

#[async_trait]
impl RateIndexer for AuraIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Aura
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Optimism,
            Chain::Polygon,
        ]
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
    async fn test_fetch_rates_ethereum() {
        let indexer = AuraIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(
            result.is_ok(),
            "Failed to fetch Aura rates: {:?}",
            result.err()
        );

        let rates = result.unwrap();
        println!(
            "Aura Ethereum: {} rates from official GraphQL API",
            rates.len()
        );
        assert!(!rates.is_empty(), "Aura should return rates");

        for rate in rates.iter().take(5) {
            println!(
                "  {} {}: Base APY {:.2}%, Rewards {:.2}%, TVL ${}",
                rate.protocol, rate.asset, rate.supply_apy, rate.rewards, rate.total_liquidity
            );
        }
    }

    #[test]
    fn test_unsupported_chain() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let indexer = AuraIndexer::new();
        let rates = rt.block_on(indexer.fetch_rates(&Chain::Solana)).unwrap();
        assert!(rates.is_empty());
    }
}
