use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Deserializer};

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;

const KAMINO_PROGRAM_ID: &str = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarketInfo {
    pub(crate) name: String,
    pub(crate) is_primary: bool,
    pub(crate) lending_market: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReserveMetrics {
    #[allow(dead_code)]
    pub(crate) reserve: String,
    pub(crate) liquidity_token: String,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub(crate) supply_apy: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub(crate) borrow_apy: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub(crate) total_supply: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub(crate) total_borrow: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub(crate) max_ltv: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StrategyInfo {
    pub(crate) strategy_name: String,
    pub(crate) pub_key: String,
    pub(crate) token_a_mint: String,
    pub(crate) token_b_mint: String,
    #[serde(default)]
    pub(crate) apy: Option<f64>,
    #[serde(default)]
    pub(crate) tvl_usd: Option<f64>,
}

fn deserialize_string_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

pub struct KaminoIndexer {
    pub client: reqwest::Client,
    pub api_url: String,
}

impl KaminoIndexer {
    pub fn new(api_url: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_url,
        }
    }

    pub async fn fetch_rates(&self) -> Result<Vec<ProtocolRate>> {
        tracing::debug!("Fetching Kamino markets from {}", self.api_url);

        // Step 1: Get list of markets
        let markets_url = format!("{}/v2/kamino-market?programId={}", self.api_url, KAMINO_PROGRAM_ID);
        let markets: Vec<MarketInfo> = self.client
            .get(&markets_url)
            .send()
            .await?
            .json()
            .await?;

        // Step 2: Find Main Market
        let main_market = markets
            .into_iter()
            .find(|m| m.is_primary && m.name == "Main Market")
            .ok_or_else(|| anyhow::anyhow!("Main Market not found in Kamino response"))?;

        tracing::debug!("Found Main Market: {}", main_market.lending_market);

        // Step 3: Get reserves metrics for Main Market
        let reserves_url = format!(
            "{}/kamino-market/{}/reserves/metrics?env=mainnet-beta",
            self.api_url,
            main_market.lending_market
        );
        
        let reserves: Vec<ReserveMetrics> = self.client
            .get(&reserves_url)
            .send()
            .await?
            .json()
            .await?;

        tracing::debug!("Fetched {} reserves from Kamino", reserves.len());

        let mut rates = self.parse_reserves(reserves);

        // Step 4: Fetch strategy vaults
        match self.fetch_strategies().await {
            Ok(mut vault_rates) => {
                tracing::info!("Fetched {} Kamino vault strategies", vault_rates.len());
                rates.append(&mut vault_rates);
            },
            Err(e) => {
                tracing::warn!("Failed to fetch Kamino strategies: {}", e);
            }
        }

        tracing::info!("Fetched {} total Kamino rates (lending + vaults)", rates.len());

        Ok(rates)
    }

    async fn fetch_strategies(&self) -> Result<Vec<ProtocolRate>> {
        let strategies_url = format!("{}/strategies?env=mainnet-beta", self.api_url);
        
        tracing::debug!("Fetching Kamino strategies from {}", strategies_url);
        
        let strategies: Vec<StrategyInfo> = self.client
            .get(&strategies_url)
            .send()
            .await?
            .json()
            .await?;

        let vault_rates = self.parse_strategies(strategies);

        Ok(vault_rates)
    }

    pub(crate) fn parse_strategies(&self, strategies: Vec<StrategyInfo>) -> Vec<ProtocolRate> {
        let mut rates = Vec::new();

        for strategy in strategies {
            // Try to identify the main asset from token mints, fallback to tokenB if tokenA is Unknown
            let asset_a = self.identify_asset_from_mint(&strategy.token_a_mint);
            let asset_b = self.identify_asset_from_mint(&strategy.token_b_mint);
            
            // Use tokenA unless it's Unknown, then try tokenB
            let asset = match asset_a {
                Asset::Known(_) => asset_a,
                Asset::Unknown(_) => asset_b,
            };

            if let Some(apy) = strategy.apy {
                let tvl = strategy.tvl_usd.unwrap_or(0.0) as u64;
                
                rates.push(ProtocolRate {
                    protocol: Protocol::Kamino,
                    chain: Chain::Solana,
                    asset,
                    action: Action::Supply,
                    supply_apy: apy * 100.0, // Convert to percentage
                    borrow_apr: 0.0,
                    rewards: 0.0,
                    performance_fee: None,
                    active: true,
                    collateral_enabled: false,  // Vaults don't support collateral
                    collateral_ltv: 0.0,
                    available_liquidity: tvl,
                    total_liquidity: tvl,
                    utilization_rate: 100.0, // Vaults typically have high utilization
                    ltv: 0.0,
                    operation_type: OperationType::Vault,
                    vault_id: Some(strategy.pub_key.clone()),
                    vault_name: Some(strategy.strategy_name),
                    underlying_asset: None,
                    timestamp: Utc::now(),
                });
            }
        }

        rates
    }

    pub(crate) fn identify_asset_from_mint(&self, mint_address: &str) -> Asset {
        // Known Solana token mint addresses - map to symbols then use from_symbol
        let symbol = match mint_address {
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC",
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => "USDT",
            "So11111111111111111111111111111111111111112" => "SOL",  // Wrapped SOL
            "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So" => "SOL",  // Marinade SOL
            "7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj" => "SOL",  // Lido stSOL
            _ => mint_address,  // Use mint address as symbol for unknown tokens
        };
        
        Asset::from_symbol(symbol, "Kamino")
    }

    pub(crate) fn parse_reserves(&self, reserves: Vec<ReserveMetrics>) -> Vec<ProtocolRate> {
        let mut rates = Vec::new();
        
        for reserve in reserves {
            let asset = Asset::from_symbol(&reserve.liquidity_token, "Kamino");
            
            let available_liquidity = (reserve.total_supply - reserve.total_borrow).max(0.0) as u64;
            let utilization_rate = if reserve.total_supply > 0.0 {
                (reserve.total_borrow / reserve.total_supply) * 100.0
            } else {
                0.0
            };

            // Supply rate
            rates.push(ProtocolRate {
                protocol: Protocol::Kamino,
                chain: Chain::Solana,
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: reserve.supply_apy * 100.0, // Convert from decimal to percentage
                borrow_apr: reserve.borrow_apy * 100.0, // Convert from decimal to percentage
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: true,
                collateral_ltv: reserve.max_ltv * 100.0,
                available_liquidity,
                total_liquidity: reserve.total_supply as u64,
                utilization_rate,
                ltv: reserve.max_ltv * 100.0, // Convert from decimal to percentage
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });
            
            // Borrow rate
            rates.push(ProtocolRate {
                protocol: Protocol::Kamino,
                chain: Chain::Solana,
                asset,
                action: Action::Borrow,
                supply_apy: reserve.supply_apy * 100.0,
                borrow_apr: reserve.borrow_apy * 100.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity,
                total_liquidity: reserve.total_supply as u64,
                utilization_rate,
                ltv: reserve.max_ltv * 100.0,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }
        
        rates
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.kamino.finance/lending".to_string()
    }
}

#[async_trait]
impl RateIndexer for KaminoIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Kamino
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Solana]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if !self.supported_chains().contains(chain) {
            return Ok(vec![]);
        }
        self.fetch_rates().await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_normalization() {
        // Tests moved to models.rs Asset::from_symbol()
    }
}
