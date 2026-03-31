use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Yearn Finance - Official yDaemon API
// ============================================================================
// Source: https://ydaemon.yearn.fi/{chainId}/vaults/all
// Supported chains: Ethereum (1), Arbitrum (42161), Base (8453), Polygon (137)
// ============================================================================

#[derive(Debug, Deserialize)]
struct YearnVault {
    symbol: String,
    name: String,
    address: String,
    #[serde(default)]
    apr: YearnApr,
    #[serde(default)]
    tvl: YearnTvl,
    #[serde(default)]
    #[allow(dead_code)]
    kind: String,
}

#[derive(Debug, Deserialize, Default)]
struct YearnApr {
    #[serde(rename = "netAPR")]
    net_apr: Option<f64>,
    #[serde(rename = "forwardAPR")]
    forward_apr: Option<YearnForwardApr>,
}

#[derive(Debug, Deserialize, Default)]
struct YearnForwardApr {
    #[serde(rename = "netAPR")]
    net_apr: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct YearnTvl {
    tvl: Option<f64>,
    #[serde(rename = "totalAssets")]
    #[allow(dead_code)]
    total_assets: Option<String>,
}

#[derive(Debug, Clone)]
pub struct YearnIndexer {
    client: reqwest::Client,
}

impl Default for YearnIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl YearnIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let chain_id = match chain {
            Chain::Ethereum => "1",
            Chain::Arbitrum => "42161",
            Chain::Base => "8453",
            Chain::Polygon => "137",
            _ => return Ok(vec![]),
        };

        tracing::info!(
            "[Yearn] Fetching vaults for chain {} from yDaemon API",
            chain_id
        );

        let url = format!("https://ydaemon.yearn.fi/{}/vaults/all", chain_id);
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            tracing::warn!("[Yearn] yDaemon API returned status: {}", resp.status());
            return Ok(vec![]);
        }

        let vaults: Vec<YearnVault> = resp.json().await?;
        let mut rates = Vec::new();

        for vault in &vaults {
            let tvl = vault.tvl.tvl.unwrap_or(0.0);
            if tvl < 1000.0 {
                continue;
            }

            // Use netAPR (historical) or forwardAPR (projected)
            let apr = vault
                .apr
                .net_apr
                .or_else(|| vault.apr.forward_apr.as_ref().and_then(|f| f.net_apr))
                .unwrap_or(0.0);

            if apr <= 0.0 || apr > 10.0 {
                continue; // Skip 0% or >1000% APR
            }

            // Extract underlying token symbol from vault symbol (yvUSDC-2 -> USDC)
            let symbol = vault
                .symbol
                .trim_start_matches("yv")
                .trim_start_matches("ys")
                .split('-')
                .next()
                .unwrap_or(&vault.symbol)
                .to_uppercase();

            let asset = Asset::from_symbol(&symbol, "Yearn");

            rates.push(ProtocolRate {
                protocol: Protocol::Yearn,
                chain: chain.clone(),
                asset,
                action: Action::Supply,
                supply_apy: (apr * 100.0 * 100.0).round() / 100.0, // apr is decimal (0.028 = 2.8%)
                borrow_apr: 0.0,
                rewards: 0.0,
                performance_fee: Some(0.10), // Yearn takes ~10% performance fee
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: tvl as u64,
                total_liquidity: tvl as u64,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Vault,
                vault_id: Some(vault.address.clone()),
                vault_name: Some(vault.name.clone()),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!(
            "[Yearn] Fetched {} vaults for {:?} (from {} total)",
            rates.len(),
            chain,
            vaults.len()
        );
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://yearn.fi/v3".to_string()
    }
}

#[async_trait]
impl RateIndexer for YearnIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Yearn
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
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
