use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::PoolIndexer;
use crate::models::{Asset, Chain, FeeTier, PoolRate, PoolType, Protocol};

// ============================================================================
// Trader Joe V2.1 (Liquidity Book) — The Graph Subgraph
// ============================================================================
// Direct data from on-chain via The Graph decentralized network.
// Liquidity Book uses bin-based concentrated liquidity (not tick-based).
// Schema entities: lbPairs, lbPairDayDatas with binStep, baseFeePct.
// Supported chains: Avalanche, Arbitrum
// ============================================================================

/// TraderJoe V2 LB subgraph IDs on The Graph decentralized network
const SUBGRAPH_IDS: &[(&str, &str)] =
    &[("avalanche", "6KD9JYCg2qa3TxNK3tLdhj5zuZTABoLLNcnUZXKG9vuH")];

#[derive(Clone)]
pub struct TraderJoeIndexer {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    #[serde(rename = "lbPairs")]
    lb_pairs: Vec<LBPair>,
}

#[derive(Debug, Deserialize)]
struct LBPair {
    id: String,
    #[serde(rename = "tokenX")]
    token_x: TokenInfo,
    #[serde(rename = "tokenY")]
    token_y: TokenInfo,
    #[serde(rename = "binStep")]
    bin_step: String,
    #[serde(rename = "baseFeePct")]
    base_fee_pct: Option<String>,
    #[serde(rename = "totalValueLockedUSD")]
    total_value_locked_usd: String,
    #[allow(dead_code)]
    #[serde(rename = "volumeUSD")]
    volume_usd: String,
    #[allow(dead_code)]
    #[serde(rename = "feesUSD")]
    fees_usd: String,
    #[serde(rename = "lbPairDayDatas")]
    lb_pair_day_datas: Option<Vec<LBPairDayData>>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    symbol: String,
    #[allow(dead_code)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct LBPairDayData {
    #[allow(dead_code)]
    date: i64,
    #[serde(rename = "volumeUSD")]
    volume_usd: String,
    #[serde(rename = "totalValueLockedUSD")]
    total_value_locked_usd: String,
    #[serde(rename = "feesUSD")]
    fees_usd: String,
}

impl TraderJoeIndexer {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            graph_api_key,
        }
    }

    pub async fn fetch_pools_for_chain(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        let chain_slug = match chain {
            Chain::Avalanche => "avalanche",
            _ => return Ok(vec![]),
        };

        let subgraph_id = match SUBGRAPH_IDS.iter().find(|(s, _)| *s == chain_slug) {
            Some((_, id)) => *id,
            None => return Ok(vec![]),
        };

        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[TraderJoe] No THE_GRAPH_API_KEY configured, skipping");
                return Ok(vec![]);
            }
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, subgraph_id
        );

        // Liquidity Book schema: lbPairs with binStep, baseFeePct, lbPairDayDatas
        let query = serde_json::json!({
            "query": r#"
            {
                lbPairs(
                    first: 200,
                    orderBy: totalValueLockedUSD,
                    orderDirection: desc,
                    where: { totalValueLockedUSD_gt: "10000" }
                ) {
                    id
                    tokenX { symbol, id }
                    tokenY { symbol, id }
                    binStep
                    baseFeePct
                    totalValueLockedUSD
                    volumeUSD
                    feesUSD
                    lbPairDayDatas(first: 7, orderBy: date, orderDirection: desc) {
                        date
                        volumeUSD
                        totalValueLockedUSD
                        feesUSD
                    }
                }
            }
            "#
        });

        tracing::info!(
            "[TraderJoe] Fetching LB pairs for {:?} from subgraph {}",
            chain,
            subgraph_id
        );

        let http_response = self.client.post(&url).json(&query).send().await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!(
                "[TraderJoe] HTTP {} for {:?}: {}",
                status,
                chain,
                &body[..body.len().min(200)]
            );
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "[TraderJoe] Failed to parse response for {:?}: {} — body: {}",
                    chain,
                    e,
                    &body[..body.len().min(200)]
                );
                return Ok(vec![]);
            }
        };

        let lb_pairs = match response.data {
            Some(data) => data.lb_pairs,
            None => {
                tracing::warn!("[TraderJoe] No data returned for {:?}", chain);
                return Ok(vec![]);
            }
        };

        tracing::info!(
            "[TraderJoe] Fetched {} LB pairs for {:?}",
            lb_pairs.len(),
            chain
        );

        let rates: Vec<PoolRate> = lb_pairs
            .into_iter()
            .filter_map(|p| Self::parse_lb_pair(p, chain))
            .collect();

        tracing::info!("[TraderJoe] Parsed {} pools for {:?}", rates.len(), chain);
        Ok(rates)
    }

    fn parse_lb_pair(pair: LBPair, chain: &Chain) -> Option<PoolRate> {
        let tvl: f64 = pair.total_value_locked_usd.parse().unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        let token0 = Asset::from_symbol(&pair.token_x.symbol, "TraderJoe");
        let token1 = Asset::from_symbol(&pair.token_y.symbol, "TraderJoe");

        // Fee: baseFeePct is a decimal percentage (e.g. "0.20" = 0.20% = 20 bps)
        // Fallback: estimate from binStep (higher binStep = higher base fee)
        let fee_rate_bps: u32 = pair
            .base_fee_pct
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|pct| (pct * 100.0).round() as u32)
            .unwrap_or_else(|| {
                let bin_step: u32 = pair.bin_step.parse().unwrap_or(20);
                // LB base fee ≈ binStep * 0.1 bps (rough approximation)
                (bin_step / 10).max(1)
            });
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        let day_data = pair.lb_pair_day_datas.unwrap_or_default();

        // 24h metrics
        let (fees_24h, volume_24h, fee_apr_24h) = if let Some(day) = day_data.first() {
            let fees: f64 = day.fees_usd.parse().unwrap_or(0.0);
            let vol: f64 = day.volume_usd.parse().unwrap_or(0.0);
            let day_tvl: f64 = day.total_value_locked_usd.parse().unwrap_or(tvl);
            let apr = if day_tvl > 0.0 {
                (fees * 365.0 / day_tvl) * 100.0
            } else {
                0.0
            };
            (fees, vol, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        // 7-day averages — extrapolate to 7 days when fewer days of data are available
        let (fees_7d, volume_7d, fee_apr_7d) = if !day_data.is_empty() {
            let days = day_data.len() as f64;
            let total_fees: f64 = day_data
                .iter()
                .map(|d| d.fees_usd.parse::<f64>().unwrap_or(0.0))
                .sum();
            let total_volume: f64 = day_data
                .iter()
                .map(|d| d.volume_usd.parse::<f64>().unwrap_or(0.0))
                .sum();
            let avg_tvl: f64 = day_data
                .iter()
                .map(|d| d.total_value_locked_usd.parse::<f64>().unwrap_or(0.0))
                .sum::<f64>()
                / days;
            let daily_avg_fees = total_fees / days;
            let daily_avg_volume = total_volume / days;
            let fees_7d = daily_avg_fees * 7.0;
            let volume_7d = daily_avg_volume * 7.0;
            let apr = if avg_tvl > 0.0 {
                (daily_avg_fees * 365.0 / avg_tvl) * 100.0
            } else {
                0.0
            };
            (fees_7d, volume_7d, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 {
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::TraderJoe,
            chain: chain.clone(),
            token0,
            token1,
            pool_type: PoolType::ConcentratedLiquidity,
            fee_tier,
            fee_rate_bps,
            tvl_usd: tvl,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr: 0.0,
            pool_address: pair.id.clone(),
            pool_id: Some(pair.id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(chain: &Chain, pool_address: &str) -> String {
        let chain_slug = match chain {
            Chain::Avalanche => "avalanche",
            Chain::Arbitrum => "arbitrum",
            _ => "avalanche",
        };
        format!(
            "https://traderjoexyz.com/{}/pool/v2/{}",
            chain_slug, pool_address
        )
    }
}

#[async_trait]
impl PoolIndexer for TraderJoeIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::TraderJoe
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Avalanche]
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        if !self.supported_chains().contains(chain) {
            return Ok(vec![]);
        }
        self.fetch_pools_for_chain(chain).await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.chain, &pool.pool_address)
    }
}
