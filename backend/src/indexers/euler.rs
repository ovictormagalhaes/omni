use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};
use super::RateIndexer;

#[derive(Debug, Serialize, Deserialize)]
struct GraphQLResponse {
    data: Option<VaultsData>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultsData {
    euler_vaults: Vec<EulerVault>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EulerVault {
    id: String,
    evault: String,
    name: String,
    symbol: String,
    asset: String,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    decimals: i32,
    #[serde(default)]
    state: Option<VaultState>,
    #[serde(default)]
    supply_cap: Option<String>,
    #[serde(default)]
    borrow_cap: Option<String>,
}

// Helper to deserialize numbers that might be strings
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Deserialize, Error};
    use serde_json::Value;
    
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => s.parse::<i32>().map_err(Error::custom),
        Value::Number(n) => n.as_i64().ok_or_else(|| Error::custom("invalid number")).map(|v| v as i32),
        _ => Err(Error::custom("expected string or number")),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultState {
    vault: String,
    total_shares: String,
    total_borrows: String,
    accumulated_fees: String,
    cash: String,
    interest_accumulator: String,
    interest_rate: String,
    supply_apy: String,
    borrow_apy: String,
    timestamp: String,
}

#[derive(Debug, Clone)]
pub struct EulerIndexer {
    client: reqwest::Client,
    graphql_url: String,
}

impl EulerIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            // Official Euler v2 subgraph on Goldsky (Ethereum Mainnet)
            graphql_url: "https://api.goldsky.com/api/public/project_cm4iagnemt1wp01xn4gh1agft/subgraphs/euler-v2-mainnet/latest/gn".to_string(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        // Euler v2 primarily operates on Ethereum
        if *chain != Chain::Ethereum {
            return Ok(Vec::new());
        }

        tracing::info!("Fetching Euler v2 rates from official Goldsky subgraph");

        let query = json!({
            "query": r#"
                query Vaults {
                    eulerVaults(
                        first: 100,
                        where: { state_: { cash_gt: "1000000000000000000" } },
                        orderBy: state__cash,
                        orderDirection: desc
                    ) {
                        id
                        evault
                        name
                        symbol
                        asset
                        decimals
                        supplyCap
                        borrowCap
                        state {
                            vault
                            totalShares
                            totalBorrows
                            accumulatedFees
                            cash
                            interestAccumulator
                            interestRate
                            supplyApy
                            borrowApy
                            timestamp
                        }
                    }
                }
            "#
        });

        let response = self
            .client
            .post(&self.graphql_url)
            .json(&query)
            .send()
            .await
            .context("Failed to send request to Euler Goldsky subgraph")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Euler Goldsky subgraph returned error {}: {}", status, body);
            return Ok(Vec::new());
        }

        let graphql_response: GraphQLResponse = response
            .json()
            .await
            .context("Failed to parse Euler Goldsky GraphQL response")?;

        match graphql_response.data {
            Some(data) => {
                let protocol_rates = self.parse_vaults(data.euler_vaults, chain)?;
                tracing::info!("Fetched {} Euler v2 vaults from Goldsky", protocol_rates.len());
                Ok(protocol_rates)
            }
            None => {
                tracing::warn!("Euler Goldsky subgraph returned no data");
                Ok(Vec::new())
            }
        }
    }

    fn parse_vaults(&self, vaults: Vec<EulerVault>, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let mut rates = Vec::new();
        let now = chrono::Utc::now();

        for vault in vaults {
            // Skip vaults without state (no activity yet)
            let state = match vault.state {
                Some(s) => s,
                None => {
                    tracing::debug!("Skipping vault {} (no state)", vault.symbol);
                    continue;
                }
            };

            // Parse asset from symbol
            // Symbol format can be: "eUSDC-80" or "ePT-USDS-14AUG2025-2" or "eweETHk-1"
            // Extract base asset: remove 'e' prefix, take first part before dash
            let asset_symbol = vault.symbol
                .trim_start_matches('e')
                .trim_start_matches('E')
                .split('-')
                .next()
                .unwrap_or("")
                .to_uppercase();
            
            // Map common assets
            let normalized_symbol = match asset_symbol.as_str() {
                "WETH" | "WEETHK" | "W" => "ETH",
                "CBBTC" => "WBTC",
                "RLUSD" | "USD0++" | "USD0" | "USDS" => "USDC", // Map stablecoins to USDC
                "PT" | "YU" => continue, // Skip PT tokens (Pendle) and YU tokens
                s => s,
            };
            
            let asset = Asset::from_symbol(normalized_symbol, "Euler");

            // Skip unknown assets
            if matches!(asset, Asset::Unknown(_)) {
                tracing::debug!("Skipping unknown/unsupported asset: {} (from vault: {})", asset_symbol, vault.symbol);
                continue;
            }

            // Parse APYs from state
            // Goldsky APY format: divide by 1e24, then divide by 2
            // Note: Goldsky appears to return the gross interest rate (100%),
            // but Euler v2 charges a protocol fee (typically 50% of interest),
            // so the net APY to depositors is approximately half.
            // Example: "15861650230762540699498849" → /1e24 → 15.86% → /2 → 7.93% (matches Euler site)
            let supply_apy_raw = state.supply_apy.parse::<f64>().unwrap_or(0.0);
            let supply_apy = (supply_apy_raw / 1e24) / 2.0; // Net APY after protocol fee
            
            let borrow_apr_raw = state.borrow_apy.parse::<f64>().unwrap_or(0.0);
            // Borrow APY comes in 1e25 format (10x higher precision than supply)
            // Example: 265219174283990000000000000 → /1e25 → 2.65% (matches Euler site)
            let borrow_apr = borrow_apr_raw / 1e25;

            // Parse cash (available liquidity) from state
            let cash = state.cash.parse::<f64>().unwrap_or(0.0);
            let decimals = vault.decimals as u32;
            let divisor = 10_f64.powi(decimals as i32);
            
            // Convert to USD (approximate - ideally fetch price from oracle)
            let available_liquidity_usd = (cash / divisor) as u64;
            
            // Parse total borrows
            let total_borrows = state.total_borrows.parse::<f64>().unwrap_or(0.0);
            let borrowed_usd = (total_borrows / divisor) as u64;
            
            // Total liquidity = cash + borrows
            let total_liquidity_usd = available_liquidity_usd + borrowed_usd;

            // Euler v2 doesn't have native rewards on most vaults
            let rewards_apy = 0.0;

            // Check if supply is active based on supply cap and liquidity
            // NOTE: Euler v2 may pause vaults administratively for security/maintenance
            // reasons that are NOT exposed in the GraphQL API. We can only detect:
            // 1. Supply cap reached (when supplyCap > 0)
            // 2. Low available liquidity (< $100k)
            // 
            // supplyCap = 0 means unlimited (no cap)
            // supplyCap > 0 means there's a cap in place
            let supply_active = if let Some(cap_str) = &vault.supply_cap {
                if let Ok(supply_cap) = cap_str.parse::<f64>() {
                    if supply_cap == 0.0 {
                        // No cap = unlimited supply allowed
                        // However, check if vault has extremely low available liquidity
                        // which might indicate operational issues
                        available_liquidity_usd > 100_000 // At least 100k available
                    } else {
                        // Has a cap - check if we're below 95% of it
                        let total_shares = state.total_shares.parse::<f64>().unwrap_or(0.0);
                        total_shares < (supply_cap * 0.95)
                    }
                } else {
                    // Can't parse cap, assume active if liquidity is good
                    available_liquidity_usd > 100_000
                }
            } else {
                // No cap info, check liquidity
                available_liquidity_usd > 100_000
            };

            // Supply action
            rates.push(ProtocolRate {
                protocol: Protocol::Euler,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy,
                borrow_apr: 0.0,
                rewards: rewards_apy,
                performance_fee: None,
                active: supply_active,
                collateral_enabled: true,  // Euler supports collateral
                collateral_ltv: 0.80,
                total_liquidity: total_liquidity_usd,
                available_liquidity: available_liquidity_usd,
                utilization_rate: if total_liquidity_usd > 0 {
                    (borrowed_usd as f64 / total_liquidity_usd as f64) * 100.0
                } else {
                    0.0
                },
                ltv: 0.80,
                operation_type: OperationType::Vault,
                vault_id: Some(vault.evault.clone()),
                vault_name: Some(vault.name.clone()),
                underlying_asset: Some(vault.asset.clone()),
                timestamp: now,
            });

            // Borrow action (if borrow APR > 0)
            if borrow_apr > 0.0 {
                // Check if borrow is active based on borrow cap and available liquidity
                // borrowCap = 0 means unlimited (no cap)
                // borrowCap > 0 means there's a cap in place
                let borrow_active = if let Some(cap_str) = &vault.borrow_cap {
                    if let Ok(borrow_cap) = cap_str.parse::<f64>() {
                        if borrow_cap == 0.0 {
                            // No cap = unlimited borrow allowed
                            // Check if there's enough liquidity available for borrowing
                            available_liquidity_usd > 100_000 // At least 100k available
                        } else {
                            // Has a cap - check if we're below 95% of it
                            let total_borrows = state.total_borrows.parse::<f64>().unwrap_or(0.0);
                            (total_borrows < (borrow_cap * 0.95)) && (available_liquidity_usd > 100_000)
                        }
                    } else {
                        // Can't parse cap, check liquidity
                        available_liquidity_usd > 100_000
                    }
                } else {
                    // No cap info, check liquidity
                    available_liquidity_usd > 100_000
                };

                rates.push(ProtocolRate {
                    protocol: Protocol::Euler,
                    chain: chain.clone(),
                    asset,
                    action: Action::Borrow,
                    supply_apy: 0.0,
                    borrow_apr,
                    rewards: 0.0,
                    performance_fee: None,
                    active: borrow_active,
                    collateral_enabled: false,  // Borrow doesn't provide collateral
                    collateral_ltv: 0.0,
                    total_liquidity: borrowed_usd,
                    available_liquidity: available_liquidity_usd,
                    utilization_rate: if total_liquidity_usd > 0 {
                        (borrowed_usd as f64 / total_liquidity_usd as f64) * 100.0
                    } else {
                        0.0
                    },
                    ltv: 0.80,
                    operation_type: OperationType::Vault,
                    vault_id: Some(vault.evault),
                    vault_name: Some(vault.name),
                    underlying_asset: Some(vault.asset),
                    timestamp: now,
                });
            }
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self, vault_address: Option<&str>) -> String {
        if let Some(addr) = vault_address {
            format!("https://app.euler.finance/vault/{}?network=ethereum", addr)
        } else {
            "https://app.euler.finance/".to_string()
        }
    }
}

#[async_trait]
impl RateIndexer for EulerIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Euler
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(rate.vault_id.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_protocol_url() {
        let indexer = EulerIndexer::new();
        let url = indexer.get_protocol_url(Some("0xa94F9CE821C7bD57cc12991CB46ca19f5789278F"));
        assert_eq!(url, "https://app.euler.finance/vault/0xa94F9CE821C7bD57cc12991CB46ca19f5789278F?network=ethereum");
        
        let default_url = indexer.get_protocol_url(None);
        assert_eq!(default_url, "https://app.euler.finance/");
    }

    #[tokio::test]
    async fn test_fetch_rates_ethereum() {
        let indexer = EulerIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        
        match result {
            Ok(rates) => {
                println!("Euler Ethereum: {} vaults", rates.len());
                for rate in rates.iter().take(3) {
                    println!("  {} {} {}: Supply APY {:.2}%, Rewards {:.2}%", 
                        rate.protocol, rate.chain, rate.asset.symbol(), 
                        rate.supply_apy, rate.rewards);
                }
            }
            Err(e) => {
                println!("Euler test failed: {:?}", e);
                // Don't panic on API failures in tests
            }
        }
    }
}