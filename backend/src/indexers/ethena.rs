use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::time::Duration;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, KnownAsset, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Ethena - Native API Integration
// ============================================================================
// sUSDe staking yield. Delta-neutral synthetic dollar with high APY.
// API: https://ethena.fi/api/yields/protocol-and-staking-yield
// Chain: Ethereum
// ============================================================================

const ETHENA_YIELD_URL: &str = "https://ethena.fi/api/yields/protocol-and-staking-yield";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthenaYieldResponse {
    staking_yield: YieldValue,
    protocol_yield: YieldValue,
    #[allow(dead_code)]
    avg30d_susde_yield: YieldValue,
}

#[derive(Debug, Deserialize)]
struct YieldValue {
    value: f64,
    #[allow(dead_code)]
    #[serde(rename = "lastUpdated")]
    last_updated: String,
}

#[derive(Debug, Clone)]
pub struct EthenaIndexer {
    pub client: reqwest::Client,
}

impl Default for EthenaIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl EthenaIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Ethereum {
            return Ok(vec![]);
        }

        tracing::info!("[Ethena] Fetching sUSDe rates from Ethena API");

        let response = self
            .client
            .get(ETHENA_YIELD_URL)
            .timeout(Duration::from_secs(30))
            .header("Accept", "application/json")
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Ethena] Failed to fetch yield data: {}", e);
                return Ok(vec![]);
            }
        };

        if !response.status().is_success() {
            tracing::warn!("[Ethena] API returned status: {}", response.status());
            return Ok(vec![]);
        }

        let yield_data: EthenaYieldResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Ethena] Failed to parse yield response: {}", e);
                return Ok(vec![]);
            }
        };

        // stakingYield = sUSDe APY (already in percentage, e.g. 3.5 = 3.5%)
        let supply_apy = yield_data.staking_yield.value;
        // protocolYield can serve as reward component if different from staking
        let rewards = (yield_data.protocol_yield.value - supply_apy).max(0.0);

        if !(-100.0..=10000.0).contains(&supply_apy) {
            tracing::warn!("[Ethena] Suspicious APY value: {}, skipping", supply_apy);
            return Ok(vec![]);
        }

        let rates = vec![ProtocolRate {
            protocol: Protocol::Ethena,
            chain: Chain::Ethereum,
            asset: Asset::Known(KnownAsset::SUSDE),
            action: Action::Supply,
            supply_apy: (supply_apy * 100.0).round() / 100.0,
            borrow_apr: 0.0,
            rewards: (rewards * 100.0).round() / 100.0,
            performance_fee: None,
            active: true,
            collateral_enabled: false,
            collateral_ltv: 0.0,
            available_liquidity: 0,
            total_liquidity: 0,
            utilization_rate: 100.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("ethena-susde-staking".to_string()),
            vault_name: Some("Ethena sUSDe Staking".to_string()),
            underlying_asset: None,
            timestamp: Utc::now(),
        }];

        tracing::info!("[Ethena] Fetched sUSDe staking APY: {:.2}%", supply_apy);
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://app.ethena.fi/stake".to_string()
    }
}

#[async_trait]
impl RateIndexer for EthenaIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Ethena
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Ethereum]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, _rate: &ProtocolRate) -> String {
        self.get_protocol_url()
    }
}
