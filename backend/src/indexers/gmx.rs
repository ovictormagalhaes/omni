use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use super::RateIndexer;
use crate::models::{Action, Asset, Chain, OperationType, Protocol, ProtocolRate};

// ============================================================================
// GMX V2 - Official API
// ============================================================================
// Source: https://arbitrum-api.gmxinfra.io (Arbitrum)
//         https://avalanche-api.gmxinfra.io (Avalanche)
// Endpoints: /markets (names), /apy?period=7d (yields), /markets/info (liquidity)
// Liquidity values use 30-decimal USD precision (divide by 1e30)
// APY values are decimals (0.05 = 5%)
// ============================================================================

#[derive(Debug, Deserialize)]
struct GmxMarketsResponse {
    markets: Vec<GmxMarket>,
}

#[derive(Debug, Deserialize)]
struct GmxMarket {
    name: Option<String>,
    #[serde(rename = "marketToken")]
    market_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmxApyEntry {
    apy: Option<f64>,
    #[serde(rename = "baseApy")]
    base_apy: Option<f64>,
    #[serde(rename = "bonusApr")]
    bonus_apr: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GmxApyResponse {
    markets: Option<HashMap<String, GmxApyEntry>>,
}

#[derive(Debug, Deserialize)]
struct GmxMarketsInfoResponse {
    markets: Option<Vec<GmxMarketInfoEntry>>,
}

#[derive(Debug, Deserialize)]
struct GmxMarketInfoEntry {
    #[serde(rename = "marketToken")]
    market_token: Option<String>,
    #[serde(rename = "availableLiquidityLong")]
    available_liquidity_long: Option<String>,
    #[serde(rename = "availableLiquidityShort")]
    available_liquidity_short: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GmxIndexer {
    pub client: reqwest::Client,
}

impl GmxIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    fn base_url(chain: &Chain) -> &'static str {
        match chain {
            Chain::Avalanche => "https://avalanche-api.gmxinfra.io",
            _ => "https://arbitrum-api.gmxinfra.io",
        }
    }

    /// Parse GMX 30-decimal USD string to f64 USD value
    fn parse_usd30(val: &str) -> f64 {
        val.parse::<f64>().unwrap_or(0.0) / 1e30
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        match chain {
            Chain::Arbitrum | Chain::Avalanche => {}
            _ => return Ok(vec![]),
        }

        let base = Self::base_url(chain);
        let timeout = std::time::Duration::from_secs(30);

        tracing::info!("[GMX] Fetching rates for {:?} from official API", chain);

        // Fetch markets, APY, and market info in parallel
        let (markets_resp, apy_resp, info_resp) = tokio::join!(
            self.client
                .get(format!("{}/markets", base))
                .timeout(timeout)
                .send(),
            self.client
                .get(format!("{}/apy?period=7d", base))
                .timeout(timeout)
                .send(),
            self.client
                .get(format!("{}/markets/info", base))
                .timeout(timeout)
                .send(),
        );

        let markets_data: GmxMarketsResponse = markets_resp?.json().await?;
        let apy_data: GmxApyResponse = apy_resp?.json().await?;

        // Market info is best-effort for liquidity data
        let liquidity_map: HashMap<String, f64> = match info_resp {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<GmxMarketsInfoResponse>().await {
                    Ok(info) => info
                        .markets
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|e| {
                            let addr = e.market_token?.to_lowercase();
                            let long = Self::parse_usd30(
                                e.available_liquidity_long.as_deref().unwrap_or("0"),
                            );
                            let short = Self::parse_usd30(
                                e.available_liquidity_short.as_deref().unwrap_or("0"),
                            );
                            Some((addr, long + short))
                        })
                        .collect(),
                    Err(e) => {
                        tracing::warn!("[GMX] Failed to parse markets/info: {}", e);
                        HashMap::new()
                    }
                }
            }
            _ => {
                tracing::warn!("[GMX] markets/info endpoint unavailable");
                HashMap::new()
            }
        };

        // Build name lookup by marketToken address
        let market_names: HashMap<String, String> = markets_data
            .markets
            .iter()
            .filter_map(|m| {
                Some((
                    m.market_token.as_ref()?.to_lowercase(),
                    m.name.clone().unwrap_or_default(),
                ))
            })
            .collect();

        let apy_markets = apy_data.markets.unwrap_or_default();
        let mut rates = Vec::new();

        for (address, data) in &apy_markets {
            let addr_lower = address.to_lowercase();
            let name = market_names
                .get(&addr_lower)
                .cloned()
                .unwrap_or_else(|| format!("GM {}", &address[..10.min(address.len())]));

            // Parse asset from market name: "ETH/USD [WETH-USDC]" -> "ETH"
            let symbol = name
                .split('/')
                .next()
                .unwrap_or(&name)
                .trim()
                .to_uppercase();
            let asset = Asset::from_symbol(&symbol, "GMX");

            // APY is decimal (0.05 = 5%), convert to percentage
            let base_apy = data.base_apy.or(data.apy).unwrap_or(0.0) * 100.0;
            let bonus = data.bonus_apr.unwrap_or(0.0) * 100.0;

            if base_apy > 10000.0 || base_apy < -100.0 {
                continue;
            }

            // Get liquidity from markets/info (30-decimal USD precision)
            let liquidity_usd = liquidity_map.get(&addr_lower).copied().unwrap_or(0.0);

            if liquidity_usd < 1000.0 && !liquidity_map.is_empty() {
                continue;
            }

            // If liquidity data unavailable, use conservative default
            let liq = if liquidity_usd > 0.0 {
                liquidity_usd as u64
            } else {
                1_000_000 // Conservative default for GMX GM pools
            };

            rates.push(ProtocolRate {
                protocol: Protocol::Gmx,
                chain: chain.clone(),
                asset,
                action: Action::Supply,
                supply_apy: (base_apy * 100.0).round() / 100.0,
                borrow_apr: 0.0,
                rewards: (bonus * 100.0).round() / 100.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,
                collateral_ltv: 0.0,
                available_liquidity: liq,
                total_liquidity: liq,
                utilization_rate: 0.0,
                ltv: 0.0,
                operation_type: OperationType::Vault,
                vault_id: Some(address.clone()),
                vault_name: Some(format!("GM {}", name)),
                underlying_asset: None,
                timestamp: Utc::now(),
            });
        }

        tracing::info!(
            "[GMX] Fetched {} rates for {:?} from official API",
            rates.len(),
            chain
        );
        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain) -> String {
        let chain_slug = match chain {
            Chain::Avalanche => "avalanche",
            _ => "arbitrum",
        };
        format!("https://app.gmx.io/#/pools?network={}", chain_slug)
    }
}

#[async_trait]
impl RateIndexer for GmxIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Gmx
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Arbitrum, Chain::Avalanche]
    }

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain)
    }
}
