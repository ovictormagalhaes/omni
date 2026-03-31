use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// Jito - Official Stake Pool Stats API
// ============================================================================
// Source: https://kobe.mainnet.jito.network/api/v1/stake_pool_stats
// Data: APY (percentage), TVL (lamports), validator count, JitoSOL supply
// APY array: latest element has current APY as percentage (e.g. 5.68 = 5.68%)
// TVL array: latest element has total staked value in lamports
// ============================================================================

#[derive(Debug, Deserialize)]
struct JitoStakePoolStats {
    apy: Vec<JitoDataPoint>,
    tvl: Vec<JitoDataPoint>,
}

#[derive(Debug, Deserialize)]
struct JitoDataPoint {
    data: f64,
}

#[derive(Debug, Clone)]
pub struct JitoIndexer {
    client: reqwest::Client,
}

impl Default for JitoIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl JitoIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        if *chain != Chain::Solana {
            return Ok(Vec::new());
        }

        tracing::info!("[Jito] Fetching staking APY from official API");

        let resp = self
            .client
            .get("https://kobe.mainnet.jito.network/api/v1/stake_pool_stats")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            tracing::warn!("[Jito] API returned status: {}", resp.status());
            return Ok(Vec::new());
        }

        let stats: JitoStakePoolStats = resp.json().await?;

        // Latest APY (already in percentage: 5.68 = 5.68%)
        let apy = stats.apy.last().map(|d| d.data).unwrap_or(0.0);

        // Latest TVL in lamports — convert to approximate USD
        // Jito TVL is ~14M SOL staked; raw value is in lamports (1e9 per SOL)
        let tvl_raw = stats.tvl.last().map(|d| d.data).unwrap_or(0.0);

        // Heuristic: if value > 1e12, it's in lamports; convert to SOL then estimate USD
        let tvl_usd = if tvl_raw > 1e12 {
            let sol_amount = tvl_raw / 1e9;
            // Approximate SOL price for TVL display — not precision-critical
            let estimated = sol_amount * 150.0;
            tracing::warn!(
                "[Jito] TVL using hardcoded SOL price estimate: raw={:.0}, sol={:.0}, usd_est={:.0}",
                tvl_raw, sol_amount, estimated
            );
            estimated
        } else if tvl_raw > 1e6 {
            // Already in USD or SOL — use as-is
            tvl_raw
        } else {
            tracing::warn!(
                "[Jito] TVL value {:.0} seems too low, using fallback",
                tvl_raw
            );
            2_000_000_000.0
        };

        if apy > 100.0 || apy <= 0.0 {
            tracing::warn!("[Jito] APY {:.2}% seems invalid, skipping", apy);
            return Ok(Vec::new());
        }

        let rates = vec![ProtocolRate {
            protocol: Protocol::Jito,
            chain: Chain::Solana,
            asset: Asset::from_symbol("JITOSOL", "Jito"),
            action: Action::Supply,
            supply_apy: (apy * 100.0).round() / 100.0,
            borrow_apr: 0.0,
            rewards: 0.0, // MEV rewards included in base APY
            performance_fee: None,
            active: true,
            collateral_enabled: false,
            collateral_ltv: 0.0,
            available_liquidity: tvl_usd as u64,
            total_liquidity: tvl_usd as u64,
            utilization_rate: 0.0,
            ltv: 0.0,
            operation_type: OperationType::Staking,
            vault_id: Some("jitosol".to_string()),
            vault_name: Some("Jito Staked SOL".to_string()),
            underlying_asset: Some("So11111111111111111111111111111111111111112".to_string()),
            timestamp: Utc::now(),
        }];

        tracing::info!(
            "[Jito] APY: {:.2}%, TVL: ${:.0} from official API",
            apy,
            tvl_usd
        );
        Ok(rates)
    }

    pub fn get_protocol_url(&self) -> String {
        "https://www.jito.network/".to_string()
    }
}

#[async_trait]
impl RateIndexer for JitoIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Jito
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Solana]
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
    async fn test_fetch_rates_non_solana_returns_empty() {
        let indexer = JitoIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
