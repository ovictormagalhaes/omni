use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

// ============================================================================
// Silo Finance V2 - Native API Integration
// ============================================================================
// Isolated lending protocol.
// APIs: https://v2.silo.finance/api/earn (supply data)
//       https://v2.silo.finance/api/borrow (borrow data + LTV)
// Supported chains: Ethereum, Arbitrum, Base, Optimism
// ============================================================================

const SILO_EARN_URL: &str = "https://v2.silo.finance/api/earn";
const SILO_BORROW_URL: &str = "https://v2.silo.finance/api/borrow";

// --- Earn API types ---

#[derive(Debug, Deserialize)]
struct EarnResponse {
    pools: Vec<EarnPool>,
    #[allow(dead_code)]
    #[serde(rename = "totalPools")]
    total_pools: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EarnPool {
    chain_key: String,
    token_symbol: String,
    #[allow(dead_code)]
    token_address: Option<String>,
    #[allow(dead_code)]
    token_decimals: Option<u32>,
    supply_apr: Option<String>,
    supply_base_apr: Option<String>,
    total_supply_usd: Option<String>,
    is_non_borrowable: Option<bool>,
    #[allow(dead_code)]
    is_market_discouraged: Option<bool>,
    market_id: Option<String>,
    programs: Option<Vec<RewardProgram>>,
}

#[derive(Debug, Deserialize)]
struct RewardProgram {
    #[allow(dead_code)]
    #[serde(rename = "rewardTokenSymbol")]
    reward_token_symbol: Option<String>,
    apr: Option<String>,
}

// --- Borrow API types ---

#[derive(Debug, Deserialize)]
struct BorrowResponse {
    pools: Vec<BorrowPool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BorrowPool {
    market_id: Option<String>,
    borrow_silo: Option<BorrowSilo>,
    supply_silo: Option<SupplySilo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BorrowSilo {
    borrow_apr: Option<String>,
    #[allow(dead_code)]
    borrow_base_apr: Option<String>,
    liquidity_usd: Option<String>,
    #[allow(dead_code)]
    is_non_borrowable: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupplySilo {
    max_ltv: Option<String>,
    #[allow(dead_code)]
    lt: Option<String>,
}

// --- Extracted borrow data for join ---

struct BorrowData {
    borrow_apr: f64,
    available_liquidity: f64,
    max_ltv: f64,
}

#[derive(Debug, Clone)]
pub struct SiloIndexer {
    pub client: reqwest::Client,
}

impl SiloIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let chain_key = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Optimism => "optimism",
            _ => {
                tracing::debug!("[Silo] Doesn't support chain {:?}, skipping", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!("[Silo] Fetching rates for {:?} from Silo V2 API", chain);

        // Fetch earn and borrow data in parallel
        let earn_body = serde_json::json!({
            "chainKeys": [chain_key],
            "type": "silo",
            "limit": 200,
            "offset": 0
        });

        let borrow_body = serde_json::json!({
            "chainKeys": [chain_key],
            "limit": 200,
            "offset": 0
        });

        let (earn_result, borrow_result) = tokio::join!(
            self.fetch_earn_data(&earn_body),
            self.fetch_borrow_data(&borrow_body),
        );

        let earn_pools = match earn_result {
            Ok(pools) => pools,
            Err(e) => {
                tracing::warn!("[Silo] Failed to fetch earn data for {:?}: {}", chain, e);
                return Ok(vec![]);
            }
        };

        // Build borrow lookup by marketId
        let borrow_map: HashMap<String, BorrowData> = match borrow_result {
            Ok(pools) => {
                pools.into_iter()
                    .filter_map(|p| {
                        let market_id = p.market_id?;
                        let borrow = p.borrow_silo?;
                        let supply = p.supply_silo.as_ref();

                        Some((market_id, BorrowData {
                            borrow_apr: wad_to_percent(borrow.borrow_apr.as_deref().unwrap_or("0")),
                            available_liquidity: raw_usd_to_f64(borrow.liquidity_usd.as_deref().unwrap_or("0")),
                            max_ltv: wad_to_decimal(supply.and_then(|s| s.max_ltv.as_deref()).unwrap_or("0")),
                        }))
                    })
                    .collect()
            }
            Err(e) => {
                tracing::warn!("[Silo] Failed to fetch borrow data for {:?}: {}, continuing with supply-only", chain, e);
                HashMap::new()
            }
        };

        tracing::debug!("[Silo] Found {} earn pools, {} borrow entries on {:?}",
            earn_pools.len(), borrow_map.len(), chain);

        let mut rates = Vec::new();

        for pool in earn_pools {
            if pool.chain_key != chain_key {
                continue;
            }

            let symbol = normalize_symbol(&pool.token_symbol);
            let asset = Asset::from_symbol(&symbol, "Silo");

            let supply_apy = wad_to_percent(pool.supply_apr.as_deref().unwrap_or("0"));
            let supply_base_apy = wad_to_percent(pool.supply_base_apr.as_deref().unwrap_or("0"));
            let total_supply = raw_usd_to_f64(pool.total_supply_usd.as_deref().unwrap_or("0"));

            // Sum reward APRs from programs
            let supply_reward: f64 = pool.programs.as_ref()
                .map(|progs| {
                    progs.iter()
                        .filter_map(|p| p.apr.as_deref().map(wad_to_percent))
                        .sum()
                })
                .unwrap_or(0.0);

            // Use base APR if available, otherwise total supply APR
            let effective_supply_apy = if supply_base_apy > 0.0 { supply_base_apy } else { supply_apy };

            // Look up borrow data by marketId
            let market_id = pool.market_id.clone().unwrap_or_default();
            let borrow_data = borrow_map.get(&market_id);

            let borrow_apr = borrow_data.map(|b| b.borrow_apr).unwrap_or(0.0);
            let available_liquidity = borrow_data.map(|b| b.available_liquidity).unwrap_or(0.0);
            let ltv = borrow_data.map(|b| b.max_ltv).unwrap_or(0.0);

            let utilization_rate = if total_supply > 0.0 {
                let total_borrow = (total_supply - available_liquidity).max(0.0);
                (total_borrow / total_supply * 100.0).min(100.0)
            } else {
                0.0
            };

            // Filters
            if total_supply < 1000.0 { continue; }
            if effective_supply_apy > 1000.0 || borrow_apr > 1000.0 { continue; }

            // Supply entry
            rates.push(ProtocolRate {
                protocol: Protocol::Silo,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: round2(effective_supply_apy),
                borrow_apr: round2(borrow_apr),
                rewards: round2(supply_reward),
                performance_fee: None,
                active: true,
                collateral_enabled: ltv > 0.0,
                collateral_ltv: ltv,
                available_liquidity: available_liquidity as u64,
                total_liquidity: total_supply as u64,
                utilization_rate,
                ltv,
                operation_type: OperationType::Lending,
                vault_id: if market_id.is_empty() { None } else { Some(market_id.clone()) },
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });

            // Borrow entry (if borrowing is enabled and there's borrow data)
            let is_non_borrowable = pool.is_non_borrowable.unwrap_or(false);
            if !is_non_borrowable && (borrow_apr > 0.0 || available_liquidity > 0.0) {
                rates.push(ProtocolRate {
                    protocol: Protocol::Silo,
                    chain: chain.clone(),
                    asset,
                    action: Action::Borrow,
                    supply_apy: round2(effective_supply_apy),
                    borrow_apr: round2(borrow_apr),
                    rewards: 0.0,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity: available_liquidity as u64,
                    total_liquidity: total_supply as u64,
                    utilization_rate,
                    ltv,
                    operation_type: OperationType::Lending,
                    vault_id: if market_id.is_empty() { None } else { Some(market_id.clone()) },
                    vault_name: None,
                    underlying_asset: None,
                    timestamp: Utc::now(),
                });
            }
        }

        tracing::info!("[Silo] Fetched {} rates for {:?}", rates.len(), chain);
        Ok(rates)
    }

    async fn fetch_earn_data(&self, body: &serde_json::Value) -> Result<Vec<EarnPool>> {
        let response = self.client
            .post(SILO_EARN_URL)
            .timeout(Duration::from_secs(30))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Silo earn API returned status: {}", response.status());
        }

        let earn_response: EarnResponse = response.json().await?;
        Ok(earn_response.pools)
    }

    async fn fetch_borrow_data(&self, body: &serde_json::Value) -> Result<Vec<BorrowPool>> {
        let response = self.client
            .post(SILO_BORROW_URL)
            .timeout(Duration::from_secs(30))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Silo borrow API returned status: {}", response.status());
        }

        let borrow_response: BorrowResponse = response.json().await?;
        Ok(borrow_response.pools)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.silo.finance/markets".to_string()
    }
}

#[async_trait]
impl RateIndexer for SiloIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Silo
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum, Chain::Arbitrum, Chain::Base, Chain::Optimism]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}

/// Convert WAD-encoded string (1e18 = 1.0) to percentage (1.0 = 1%)
fn wad_to_percent(wad: &str) -> f64 {
    let raw: f64 = wad.parse().unwrap_or(0.0);
    (raw / 1e18) * 100.0
}

/// Convert WAD-encoded string (1e18 = 1.0) to decimal (0.92 = 92%)
fn wad_to_decimal(wad: &str) -> f64 {
    let raw: f64 = wad.parse().unwrap_or(0.0);
    raw / 1e18
}

/// Convert raw USD string to f64 dollars (values appear to be in micro-dollars / 1e6 precision)
fn raw_usd_to_f64(raw: &str) -> f64 {
    let value: f64 = raw.parse().unwrap_or(0.0);
    value / 1e6
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.to_uppercase();
    s.split(|c: char| c == '-' || c == '/' || c == ' ')
        .next()
        .unwrap_or(&s)
        .to_string()
}
