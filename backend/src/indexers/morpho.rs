use crate::models::{Asset, Chain, Protocol};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

// GraphQL response for vaults query
#[derive(Debug, Serialize, Deserialize)]
struct VaultsGraphQLResponse {
    data: Option<VaultsData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultsData {
    vaults: VaultsResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultsResult {
    items: Vec<Vault>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vault {
    address: String,
    name: String,
    symbol: String,
    chain: VaultChain,
    asset: VaultAsset,
    state: VaultState,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultChain {
    id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultAsset {
    address: String,
    symbol: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultState {
    #[serde(default, deserialize_with = "deserialize_apy_safe")]
    net_apy: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_apy_safe")]
    apy: Option<f64>,
    #[serde(default)]
    fee: Option<f64>,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    total_assets: String,
    total_assets_usd: Option<f64>,
}

// Legacy Markets support (kept for reference, not used)
#[derive(Debug, Serialize, Deserialize)]
struct GraphQLResponse {
    data: Option<MarketsData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarketsData {
    markets: MarketsResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarketsResult {
    items: Vec<Market>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Market {
    unique_key: String,
    loan_asset: TokenInfo,
    collateral_asset: Option<TokenInfo>,
    state: MarketState,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenInfo {
    symbol: String,
    address: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarketState {
    supply_apy: f64,
    borrow_apy: f64,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    supply_assets: String,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    borrow_assets: String,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    liquidity_assets: String,
    supply_assets_usd: f64,
    borrow_assets_usd: f64,
    liquidity_assets_usd: f64,
    utilization: f64,
    #[serde(default)]
    rewards: Option<Vec<Reward>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reward {
    #[serde(default)]
    supply_apy: Option<f64>,
    #[serde(default)]
    borrow_apy: Option<f64>,
    asset: RewardAsset,
}

#[derive(Debug, Serialize, Deserialize)]
struct RewardAsset {
    symbol: String,
}

// Helper to deserialize numbers that might be strings or integers
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Deserialize, Error};
    use serde_json::Value;
    
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(Error::custom("expected string or number")),
    }
}

/// Deserialize APY safely - accepts string or number, validates range
/// Returns None for invalid/extreme values to skip corrupted data
fn deserialize_apy_safe<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;
    use serde_json::Value;
    
    let value = Value::deserialize(deserializer)?;
    let apy_opt: Option<f64> = match value {
        Value::Null => None,
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            // Try to parse string as f64, but truncate extreme values
            match s.parse::<f64>() {
                Ok(v) => Some(v),
                Err(_) => {
                    // If string is too large to parse (e.g., 2.417806...e200), return None
                    tracing::warn!("Morpho API returned unparseable APY string (too large): {} chars", s.len());
                    None
                }
            }
        }
        _ => None,
    };
    
    // Validate APY is reasonable (< 10,000% = 100.0 in decimal)
    // Morpho API returns decimals (0.05 = 5%), so 100.0 = 10,000%
    // Note: High-risk vaults can have extreme but legitimate APYs (e.g., 298,000%)
    // We only filter truly corrupted data (> 1,000,000%)
    match apy_opt {
        Some(apy) if apy.is_finite() && apy >= -1.0 && apy <= 10000.0 => {
            if apy > 1.0 {
                tracing::info!("Morpho vault with high APY: {:.2}% (may have security risks)", apy * 100.0);
            }
            Ok(Some(apy))
        }
        Some(apy) => {
            tracing::error!("Morpho API returned corrupted APY: {} - skipping vault", apy);
            Ok(None)
        }
        None => Ok(None),
    }
}

pub struct MorphoIndexer {
    pub client: reqwest::Client,
    pub api_url: String,
}

impl MorphoIndexer {
    pub fn new(api_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
        }
    }

    pub async fn fetch_rates(&self) -> Result<Vec<crate::models::ProtocolRate>> {
        let mut all_rates = Vec::new();

        // Fetch rates for supported chains
        let chains = vec![
            (Chain::Ethereum, 1),
            (Chain::Arbitrum, 42161),
            (Chain::Base, 8453),
            (Chain::Polygon, 137),
            (Chain::Optimism, 10),
        ];

        for (chain, chain_id) in chains {
            match self.fetch_chain_rates(&chain, chain_id).await {
                Ok(mut rates) => all_rates.append(&mut rates),
                Err(e) => {
                    tracing::warn!("Failed to fetch Morpho rates for chain {:?}: {:?}", chain, e);
                }
            }
        }

        Ok(all_rates)
    }

    async fn fetch_chain_rates(
        &self,
        chain: &Chain,
        chain_id: u32,
    ) -> Result<Vec<crate::models::ProtocolRate>> {
        // Query MetaMorpho Vaults instead of Markets
        let query = json!({
            "query": r#"
                query Vaults($chainId: Int!) {
                    vaults(
                        first: 100
                        where: { chainId_in: [$chainId] }
                    ) {
                        items {
                            address
                            name
                            symbol
                            chain {
                                id
                            }
                            asset {
                                address
                                symbol
                            }
                            state {
                                netApy
                                apy
                                fee
                                totalAssets
                                totalAssetsUsd
                            }
                        }
                    }
                }
            "#,
            "variables": {
                "chainId": chain_id
            }
        });

        let response = self
            .client
            .post(&self.api_url)
            .json(&query)
            .send()
            .await
            .context("Failed to send GraphQL request to Morpho API")?;

        let graphql_response: VaultsGraphQLResponse = response
            .json()
            .await
            .context("Failed to parse GraphQL response from Morpho API")?;

        let vaults = graphql_response
            .data
            .context("No data in GraphQL response")?
            .vaults
            .items;

        let mut rates = Vec::new();
        let now = Utc::now();

        for vault in vaults {
            // Skip vaults with missing critical data
            let net_apy = match vault.state.net_apy {
                Some(apy) if !apy.is_nan() && apy.is_finite() => apy,
                _ => {
                    tracing::debug!("Skipping Morpho vault {} - invalid net_apy", vault.name);
                    continue;
                }
            };
            
            let base_apy = vault.state.apy.unwrap_or(0.0);
            let total_assets_usd = vault.state.total_assets_usd.unwrap_or(0.0);
            
            // Skip vaults with zero TVL
            if total_assets_usd < 1.0 {
                tracing::debug!("Skipping Morpho vault {} - TVL too low: ${}", vault.name, total_assets_usd);
                continue;
            }
            
            // Map asset symbol to our Asset enum
            let asset = Asset::from_symbol(&vault.asset.symbol, "Morpho");
            
            // Morpho vaults only have supply-side APY (no borrowing from vaults)
            // Morpho API formula: netApy = apy + rewards - (apy * fee)
            // Therefore: rewards = netApy - apy + (apy * fee)
            let net_apy_percent = net_apy * 100.0; // Convert to percentage
            let base_apy_percent = base_apy * 100.0;
            let vault_fee = vault.state.fee.unwrap_or(0.0);
            let fee_impact_percent = base_apy_percent * vault_fee;
            let rewards_apy = (net_apy_percent - base_apy_percent + fee_impact_percent).max(0.0);
            
            // Parse total assets
            let total_assets_usd_u64 = total_assets_usd as u64;
            let utilization_rate = 0.0; // allocation is complex, not available
            
            // Consider vault active if TVL > $1000 and APY is reasonable
            let is_active = total_assets_usd >= 1000.0 && base_apy_percent >= 0.0 && base_apy_percent < 1000.0;

            rates.push(crate::models::ProtocolRate {
                protocol: Protocol::Morpho,
                chain: chain.clone(),
                asset: asset.clone(),
                action: crate::models::Action::Supply,
                supply_apy: base_apy_percent,  // Use base APY (aggregator will add rewards)
                borrow_apr: 0.0, // Vaults don't have borrowing
                rewards: rewards_apy,
                performance_fee: Some(vault_fee),
                active: is_active,
                collateral_enabled: false,  // Morpho vaults don't support collateral
                collateral_ltv: 0.0,
                total_liquidity: total_assets_usd_u64,
                available_liquidity: total_assets_usd_u64, // Approximation
                utilization_rate: utilization_rate,
                ltv: 0.0, // Not applicable for vaults
                operation_type: crate::models::OperationType::Vault,
                vault_id: Some(vault.address.clone()),
                vault_name: Some(vault.name.clone()),
                underlying_asset: Some(vault.asset.symbol.clone()),
                timestamp: now,
            });
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain, vault_id: Option<&str>) -> String {
        let network = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Optimism => "optimism",
            _ => "ethereum",
        };
        
        if let Some(id) = vault_id {
            format!("https://app.morpho.org/{}/vault/{}", network, id)
        } else {
            format!("https://app.morpho.org/{}/earn", network)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_vaults_v1_response_without_rewards() {
        // Mock response from vaults endpoint (v1) - no rewards field
        let json_response = json!({
            "data": {
                "vaults": {
                    "items": [
                        {
                            "address": "0xb0f05E4De970A1aaf77f8C2F823953a367504BA9",
                            "name": "ALPHA USDC Core",
                            "symbol": "aUSDC",
                            "chain": {
                                "id": 1
                            },
                            "asset": {
                                "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                                "symbol": "USDC"
                            },
                            "state": {
                                "netApy": 0.05857524427170772,
                                "apy": 0.06050315974722611,
                                "fee": 0.1,
                                "totalAssets": "17681995146222",
                                "totalAssetsUsd": 17678468.54263729
                            }
                        }
                    ]
                }
            }
        });

        let response: VaultsGraphQLResponse = serde_json::from_value(json_response).unwrap();
        assert!(response.data.is_some());
        
        let vaults = response.data.unwrap().vaults.items;
        assert_eq!(vaults.len(), 1);
        
        let vault = &vaults[0];
        assert_eq!(vault.address, "0xb0f05E4De970A1aaf77f8C2F823953a367504BA9");
        assert_eq!(vault.name, "ALPHA USDC Core");
        assert_eq!(vault.asset.symbol, "USDC");
        assert_eq!(vault.chain.id, 1);
        
        // Verify APY values
        assert!(vault.state.net_apy.is_some());
        assert!(vault.state.apy.is_some());
        assert_eq!(vault.state.fee, Some(0.1));
        
        let net_apy = vault.state.net_apy.unwrap();
        let apy = vault.state.apy.unwrap();
        
        // Calculate rewards using the reverse engineering formula
        let net_apy_percent = net_apy * 100.0;
        let base_apy_percent = apy * 100.0;
        let vault_fee = vault.state.fee.unwrap_or(0.0);
        let fee_impact_percent = base_apy_percent * vault_fee;
        let rewards_apy = (net_apy_percent - base_apy_percent + fee_impact_percent).max(0.0);
        
        // Expected: (5.857 - 6.050 + 0.605) = 0.412%
        assert!((rewards_apy - 0.412).abs() < 0.01, "Rewards APY should be ~0.412%, got {}", rewards_apy);
    }

    #[test]
    fn test_vaultv2s_structure_with_rewards() {
        // Structure for vaultV2s endpoint (v2) - has rewards field
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct VaultV2 {
            address: String,
            asset: VaultAsset,
            avg_apy: f64,
            avg_net_apy: f64,
            performance_fee: f64,
            rewards: Vec<VaultReward>,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct VaultReward {
            asset: RewardAssetInfo,
            supply_apr: f64,
        }

        #[derive(Debug, Deserialize)]
        struct RewardAssetInfo {
            symbol: String,
        }

        // Mock response from vaultV2s endpoint with MORPHO rewards
        let json_response = json!({
            "address": "0x0229dB3921dE71CFa43Cfe9fb6A87b403647A9ae",
            "asset": {
                "symbol": "USDC"
            },
            "avgApy": 0.0332,
            "avgNetApy": 0.0386,
            "performanceFee": 0.0,
            "rewards": [
                {
                    "asset": {
                        "symbol": "MORPHO"
                    },
                    "supplyApr": 0.0054
                }
            ]
        });

        let vault: VaultV2 = serde_json::from_value(json_response).unwrap();
        
        assert_eq!(vault.address, "0x0229dB3921dE71CFa43Cfe9fb6A87b403647A9ae");
        assert_eq!(vault.asset.symbol, "USDC");
        assert_eq!(vault.rewards.len(), 1);
        assert_eq!(vault.rewards[0].asset.symbol, "MORPHO");
        assert!((vault.rewards[0].supply_apr - 0.0054).abs() < 0.0001);
    }

    #[test]
    fn test_deserialize_apy_safe_valid_values() {
        // Test valid APY values
        let json_normal = json!({"apy": 0.05});
        let parsed: serde_json::Value = json_normal;
        
        // Simulate normal APY parsing through VaultState
        let vault_state_json = json!({
            "netApy": 0.05,
            "apy": 0.06,
            "totalAssets": "1000000",
            "totalAssetsUsd": 1000000.0
        });
        
        let state: VaultState = serde_json::from_value(vault_state_json).unwrap();
        assert!(state.net_apy.is_some());
        assert!(state.apy.is_some());
        assert_eq!(state.net_apy.unwrap(), 0.05);
        assert_eq!(state.apy.unwrap(), 0.06);
    }

    #[test]
    fn test_deserialize_apy_safe_extreme_values() {
        // Test extreme/corrupted APY values that should be filtered
        let vault_state_json = json!({
            "netApy": 99999.0,  // Extreme value (> 10000)
            "apy": 0.06,
            "totalAssets": "1000000",
            "totalAssetsUsd": 1000000.0
        });
        
        let state: VaultState = serde_json::from_value(vault_state_json).unwrap();
        assert!(state.net_apy.is_none(), "Extreme APY should be filtered out");
    }

    #[test]
    fn test_deserialize_apy_safe_negative_values() {
        // Test negative APY (should be allowed within range)
        let vault_state_json = json!({
            "netApy": -0.01,  // -1% is allowed
            "apy": 0.0,
            "totalAssets": "1000000",
            "totalAssetsUsd": 1000000.0
        });
        
        let state: VaultState = serde_json::from_value(vault_state_json).unwrap();
        assert!(state.net_apy.is_some());
        assert_eq!(state.net_apy.unwrap(), -0.01);
    }

    #[test]
    fn test_deserialize_apy_safe_null_values() {
        // Test null APY values
        let vault_state_json = json!({
            "netApy": null,
            "apy": null,
            "totalAssets": "1000000",
            "totalAssetsUsd": 1000000.0
        });
        
        let state: VaultState = serde_json::from_value(vault_state_json).unwrap();
        assert!(state.net_apy.is_none());
        assert!(state.apy.is_none());
    }

    #[test]
    fn test_rewards_calculation_formula() {
        // Test the reverse engineering formula for rewards
        // Formula: rewards = netApy - apy + (apy * fee)
        
        struct TestCase {
            net_apy: f64,
            apy: f64,
            fee: f64,
            expected_rewards: f64,
        }
        
        let test_cases = vec![
            // ALPHA USDC Core example
            TestCase {
                net_apy: 5.857,
                apy: 6.050,
                fee: 0.1,
                expected_rewards: 0.412, // 5.857 - 6.050 + 0.605
            },
            // Higher fee example
            TestCase {
                net_apy: 4.5,
                apy: 5.0,
                fee: 0.15,
                expected_rewards: 0.25, // 4.5 - 5.0 + 0.75
            },
            // No fee example
            TestCase {
                net_apy: 4.0,
                apy: 5.0,
                fee: 0.0,
                expected_rewards: 0.0, // max(4.0 - 5.0 + 0, 0) = 0
            },
        ];
        
        for case in test_cases {
            let fee_impact = case.apy * case.fee;
            let calculated_rewards = (case.net_apy - case.apy + fee_impact).max(0.0);
            
            assert!(
                (calculated_rewards - case.expected_rewards).abs() < 0.01,
                "Expected rewards {}, got {} for netApy={}, apy={}, fee={}",
                case.expected_rewards,
                calculated_rewards,
                case.net_apy,
                case.apy,
                case.fee
            );
        }
    }

    #[test]
    fn test_deserialize_string_or_number() {
        // Test totalAssets can be string or number
        let vault_state_string = json!({
            "netApy": 0.05,
            "apy": 0.06,
            "totalAssets": "17681995146222",
            "totalAssetsUsd": 17678468.54
        });
        
        let state: VaultState = serde_json::from_value(vault_state_string).unwrap();
        assert_eq!(state.total_assets, "17681995146222");
        
        // Test with numeric totalAssets
        let vault_state_number = json!({
            "netApy": 0.05,
            "apy": 0.06,
            "totalAssets": 17681995146222u64,
            "totalAssetsUsd": 17678468.54
        });
        
        let state2: VaultState = serde_json::from_value(vault_state_number).unwrap();
        assert_eq!(state2.total_assets, "17681995146222");
    }

    #[test]
    fn test_morpho_url_generation() {
        let indexer = MorphoIndexer::new("https://api.morpho.org/graphql".to_string());
        
        // Test Ethereum vault URL
        let url = indexer.get_protocol_url(
            &Chain::Ethereum,
            Some("0xb0f05E4De970A1aaf77f8C2F823953a367504BA9")
        );
        assert_eq!(
            url,
            "https://app.morpho.org/ethereum/vault/0xb0f05E4De970A1aaf77f8C2F823953a367504BA9"
        );
        
        // Test Arbitrum vault URL
        let url_arb = indexer.get_protocol_url(
            &Chain::Arbitrum,
            Some("0x1234567890abcdef")
        );
        assert_eq!(
            url_arb,
            "https://app.morpho.org/arbitrum/vault/0x1234567890abcdef"
        );
        
        // Test without vault ID
        let url_base = indexer.get_protocol_url(&Chain::Base, None);
        assert_eq!(url_base, "https://app.morpho.org/base/earn");
    }

    #[test]
    fn test_vault_with_zero_rewards() {
        // Test vault with no rewards (netApy == apy after fee)
        let json_response = json!({
            "data": {
                "vaults": {
                    "items": [
                        {
                            "address": "0x01afD5fa09b5664E39b5D064E0C21BAd274E4d8",
                            "name": "Steakhouse USDC High Yield",
                            "symbol": "stkUSDC",
                            "chain": {
                                "id": 1
                            },
                            "asset": {
                                "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                                "symbol": "USDC"
                            },
                            "state": {
                                "netApy": 0.0412,
                                "apy": 0.0486,
                                "fee": 0.15,
                                "totalAssets": "10000000000000",
                                "totalAssetsUsd": 10000000.0
                            }
                        }
                    ]
                }
            }
        });

        let response: VaultsGraphQLResponse = serde_json::from_value(json_response).unwrap();
        let vault = &response.data.unwrap().vaults.items[0];
        
        let net_apy = vault.state.net_apy.unwrap() * 100.0;
        let apy = vault.state.apy.unwrap() * 100.0;
        let fee = vault.state.fee.unwrap();
        
        let fee_impact = apy * fee;
        let rewards = (net_apy - apy + fee_impact).max(0.0);
        
        // Expected: 4.12 - 4.86 + 0.729 = -0.011 -> max(0) = 0.0
        assert!(rewards < 0.1, "Vault without rewards should calculate ~0%, got {}", rewards);
    }
}
