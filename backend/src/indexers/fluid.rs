use super::RateIndexer;
use crate::models::{Asset, Chain, Protocol, ProtocolRate};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidLendingResponse {
    data: Vec<FluidTokenResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidTokenResponse {
    asset: AssetInfo,
    #[serde(deserialize_with = "deserialize_string_or_float")]
    supply_rate: f64,
    #[serde(default)]
    rewards: Vec<FluidReward>,
    #[serde(deserialize_with = "deserialize_string_or_float")]
    total_rate: f64,
    total_assets: String,
    total_supply: String,
    liquidity_supply_data: Option<LiquiditySupplyData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FluidReward {
    #[serde(deserialize_with = "deserialize_string_or_float")]
    rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AssetInfo {
    symbol: String,
    address: String,
    decimals: u32,
    price: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiquiditySupplyData {
    supply: String,
    withdrawable: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidVaultResponse {
    vault_id: u32,
    vault_address: String,
    supply_token: VaultToken,
    borrow_token: VaultToken,
    supply_rate: VaultRate,
    borrow_rate: VaultRate,
    liquidity: VaultLiquidity,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultToken {
    token0: TokenDetail,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenDetail {
    symbol: String,
    address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultRate {
    vault: VaultRateDetail,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultRateDetail {
    #[serde(deserialize_with = "deserialize_string_or_float")]
    rate: f64,
}

// Helper to deserialize float that might be a string
fn deserialize_string_or_float<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Deserialize, Error};
    use serde_json::Value;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => s.parse::<f64>().map_err(Error::custom),
        Value::Number(n) => n.as_f64().ok_or_else(|| Error::custom("invalid number")),
        _ => Err(Error::custom("expected string or number")),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultLiquidity {
    utilization: f64,
    total_supply_usd: f64,
    total_borrow_usd: f64,
}

pub struct FluidIndexer {
    pub client: reqwest::Client,
    pub api_url: String,
}

impl FluidIndexer {
    pub fn new(api_url: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_url,
        }
    }

    pub async fn fetch_rates(&self) -> Result<Vec<crate::models::ProtocolRate>> {
        let mut all_rates = Vec::new();

        // Fluid is currently only on Ethereum Mainnet (chain-id: 1)
        // Only fetch lending rates (supply) as vaults structure is complex
        match self.fetch_lending_rates().await {
            Ok(mut rates) => all_rates.append(&mut rates),
            Err(e) => {
                tracing::warn!("Failed to fetch Fluid lending rates: {:?}", e);
            }
        }

        Ok(all_rates)
    }

    pub async fn fetch_lending_rates(&self) -> Result<Vec<crate::models::ProtocolRate>> {
        let url = format!("{}/v2/lending/1/tokens", self.api_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Fluid lending tokens")?;

        let response_text = response.text().await?;
        tracing::debug!(
            "Fluid API response (first 500 chars): {}",
            &response_text[..response_text.len().min(500)]
        );

        // API returns {"data": [...], "totalAssetsInUsd": "...", ...}
        let fluid_response: FluidLendingResponse = serde_json::from_str(&response_text)
            .context("Failed to parse Fluid lending response")?;

        let tokens = fluid_response.data;

        let mut rates = Vec::new();
        let now = Utc::now();

        for token in tokens {
            let asset = Asset::from_symbol(&token.asset.symbol, "Fluid");

            // Parse price and calculate USD values
            let price: f64 = token.asset.price.parse().unwrap_or(0.0);
            let decimals = token.asset.decimals;

            // Parse total assets (in token units with decimals)
            let total_assets: u128 = token.total_assets.parse().unwrap_or(0);
            let total_supply: u128 = token.total_supply.parse().unwrap_or(0);

            // Calculate liquidity in USD
            let divisor = 10_u128.pow(decimals);
            let total_liquidity_usd = ((total_assets as f64 / divisor as f64) * price) as u64;
            let supplied_liquidity_usd = ((total_supply as f64 / divisor as f64) * price) as u64;

            // Calculate available liquidity
            let available_liquidity = if let Some(ref liq_data) = token.liquidity_supply_data {
                let withdrawable: u128 = liq_data.withdrawable.parse().unwrap_or(0);
                ((withdrawable as f64 / divisor as f64) * price) as u64
            } else {
                total_liquidity_usd.saturating_sub(supplied_liquidity_usd)
            };

            // Calculate utilization rate
            let utilization_rate = if total_liquidity_usd > 0 {
                ((supplied_liquidity_usd as f64 / total_liquidity_usd as f64) * 100.0) as u32
            } else {
                0
            };

            // Supply rate (Fluid returns basis points, divide by 100 to get percentage)
            let supply_apy = token.supply_rate / 100.0;

            // Sum all rewards from the rewards array
            let rewards_apy: f64 = token.rewards.iter().map(|r| r.rate).sum::<f64>() / 100.0;

            rates.push(crate::models::ProtocolRate {
                protocol: Protocol::Fluid,
                chain: Chain::Ethereum,
                asset: asset.clone(),
                action: crate::models::Action::Supply,
                supply_apy: supply_apy,
                borrow_apr: 0.0,
                rewards: rewards_apy,
                performance_fee: None,
                active: true,
                collateral_enabled: true, // Fluid supports collateral
                collateral_ltv: 0.75,
                total_liquidity: total_liquidity_usd,
                available_liquidity,
                utilization_rate: utilization_rate as f64,
                ltv: 0.75,
                operation_type: crate::models::OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }

    pub async fn fetch_vault_rates(&self) -> Result<Vec<crate::models::ProtocolRate>> {
        let url = format!("{}/v2/borrowing/1/vaults", self.api_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Fluid vaults")?;

        let vaults: Vec<FluidVaultResponse> = response
            .json()
            .await
            .context("Failed to parse Fluid vault response")?;

        let mut rates = Vec::new();
        let now = Utc::now();

        for vault in vaults {
            // Process supply token
            let asset = Asset::from_symbol(&vault.supply_token.token0.symbol, "Fluid");

            let utilization_rate = (vault.liquidity.utilization * 100.0) as u32;
            let total_supply = vault.liquidity.total_supply_usd as u64;
            let total_borrow = vault.liquidity.total_borrow_usd as u64;
            let available_liquidity = total_supply.saturating_sub(total_borrow);

            // Borrow rate (Fluid returns basis points, divide by 100 to get percentage)
            let borrow_rate = vault.borrow_rate.vault.rate / 100.0;

            rates.push(crate::models::ProtocolRate {
                protocol: Protocol::Fluid,
                chain: Chain::Ethereum,
                asset,
                action: crate::models::Action::Borrow,
                supply_apy: 0.0,
                borrow_apr: borrow_rate,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false, // Borrow doesn't provide collateral
                collateral_ltv: 0.0,
                total_liquidity: total_borrow,
                available_liquidity,
                utilization_rate: utilization_rate as f64,
                ltv: 0.75,
                operation_type: crate::models::OperationType::Vault,
                vault_id: None,
                vault_name: None,
                underlying_asset: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain, asset: &Asset) -> String {
        let chain_id = match chain {
            Chain::Ethereum => "1",
            Chain::Base => "8453",
            Chain::Arbitrum => "42161",
            _ => return "https://fluid.io".to_string(),
        };
        format!("https://fluid.io/lending/{}/{}", chain_id, asset)
    }
}

#[async_trait]
impl RateIndexer for FluidIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Fluid
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if !self.supported_chains().contains(chain) {
            return Ok(vec![]);
        }
        self.fetch_rates().await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain, &rate.asset)
    }
}
