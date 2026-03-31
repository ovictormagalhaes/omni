use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use crate::models::{Asset, Chain, Protocol, PoolRate, PoolType, FeeTier};
use super::PoolIndexer;

// ============================================================================
// Aerodrome — The Graph Subgraph (Solidly/Velodrome fork)
// ============================================================================
// Direct data from on-chain via The Graph decentralized network.
// Aerodrome is the primary DEX on Base (fork of Velodrome/Solidly).
// Subgraph uses Solidly-style schema: pairs, pairDayDatas, gauges.
// ============================================================================

const SUBGRAPH_ID: &str = "GENunSHWLBXm59mBSgPzQ8metBEp9YDfdqwFr91Av1UM";

#[derive(Clone)]
pub struct AerodromeIndexer {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

// GraphQL response structures (Solidly/Velodrome schema)

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    pairs: Option<Vec<SubgraphPair>>,
    pools: Option<Vec<SubgraphPair>>,
}

#[derive(Debug, Deserialize)]
struct SubgraphPair {
    id: String,
    token0: TokenInfo,
    token1: TokenInfo,
    #[serde(rename = "isStable")]
    is_stable: Option<bool>,
    stable: Option<bool>,
    #[serde(rename = "reserveUSD")]
    reserve_usd: Option<String>,
    #[serde(rename = "totalValueLockedUSD")]
    total_value_locked_usd: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "volumeUSD")]
    volume_usd: Option<String>,
    #[serde(rename = "feeTier")]
    fee_tier: Option<String>,
    fee: Option<String>,
    #[serde(rename = "pairDayData")]
    pair_day_data: Option<Vec<DayData>>,
    #[serde(rename = "poolDayData")]
    pool_day_data: Option<Vec<DayData>>,
    #[allow(dead_code)]
    gauge: Option<GaugeInfo>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    symbol: String,
    #[allow(dead_code)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct DayData {
    #[allow(dead_code)]
    date: Option<i64>,
    #[serde(rename = "volumeUSD")]
    volume_usd: Option<String>,
    #[serde(rename = "reserveUSD")]
    reserve_usd: Option<String>,
    #[serde(rename = "tvlUSD")]
    tvl_usd: Option<String>,
    #[serde(rename = "feesUSD")]
    fees_usd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GaugeInfo {
    #[serde(rename = "rewardRate")]
    reward_rate: Option<String>,
    #[serde(rename = "totalSupply")]
    total_supply: Option<String>,
}

impl AerodromeIndexer {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            graph_api_key,
        }
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[Aerodrome] No THE_GRAPH_API_KEY configured, skipping");
                return Ok(vec![]);
            }
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, SUBGRAPH_ID
        );

        // Try Solidly-style schema first (pairs), fallback to Uni-style (pools)
        let rates = self.try_fetch_solidly(&url).await?;
        if !rates.is_empty() {
            return Ok(rates);
        }

        // Fallback: try Uni V3-style schema
        self.try_fetch_uni_style(&url).await
    }

    async fn try_fetch_solidly(&self, url: &str) -> Result<Vec<PoolRate>> {
        let query = serde_json::json!({
            "query": r#"
            {
                pairs(
                    first: 200,
                    orderBy: reserveUSD,
                    orderDirection: desc,
                    where: { reserveUSD_gt: "10000" }
                ) {
                    id
                    token0 { symbol, id }
                    token1 { symbol, id }
                    isStable
                    stable
                    reserveUSD
                    volumeUSD
                    fee
                    pairDayData(first: 7, orderBy: date, orderDirection: desc) {
                        date
                        volumeUSD
                        reserveUSD
                        feesUSD
                    }
                    gauge {
                        rewardRate
                        totalSupply
                    }
                }
            }
            "#
        });

        tracing::info!("[Aerodrome] Fetching pairs (Solidly schema) from subgraph");
        self.execute_query(url, &query, true).await
    }

    async fn try_fetch_uni_style(&self, url: &str) -> Result<Vec<PoolRate>> {
        let query = serde_json::json!({
            "query": r#"
            {
                pools(
                    first: 200,
                    orderBy: totalValueLockedUSD,
                    orderDirection: desc,
                    where: { totalValueLockedUSD_gt: "10000" }
                ) {
                    id
                    token0 { symbol, id }
                    token1 { symbol, id }
                    feeTier
                    totalValueLockedUSD
                    volumeUSD
                    poolDayData(first: 7, orderBy: date, orderDirection: desc) {
                        date
                        volumeUSD
                        tvlUSD
                        feesUSD
                    }
                }
            }
            "#
        });

        tracing::info!("[Aerodrome] Fetching pools (Uni-style schema) from subgraph");
        self.execute_query(url, &query, false).await
    }

    async fn execute_query(&self, url: &str, query: &serde_json::Value, is_solidly: bool) -> Result<Vec<PoolRate>> {
        let http_response = self.client
            .post(url)
            .json(query)
            .send()
            .await?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            tracing::warn!("[Aerodrome] HTTP {} : {}", status, &body[..body.len().min(300)]);
            return Ok(vec![]);
        }

        let body = http_response.text().await?;
        let response: GraphQLResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Aerodrome] Failed to parse response: {} — body: {}", e, &body[..body.len().min(300)]);
                return Ok(vec![]);
            }
        };

        let data = match response.data {
            Some(d) => d,
            None => {
                tracing::warn!("[Aerodrome] No data returned, schema may not match");
                return Ok(vec![]);
            }
        };

        let items = if is_solidly {
            data.pairs.unwrap_or_default()
        } else {
            data.pools.unwrap_or_default()
        };

        if items.is_empty() {
            return Ok(vec![]);
        }

        tracing::info!("[Aerodrome] Fetched {} items from subgraph", items.len());

        let rates: Vec<PoolRate> = items
            .into_iter()
            .filter_map(|p| Self::parse_pair(p, is_solidly))
            .collect();

        tracing::info!("[Aerodrome] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pair(pair: SubgraphPair, is_solidly: bool) -> Option<PoolRate> {
        let token0 = Asset::from_symbol(&pair.token0.symbol, "Aerodrome");
        let token1 = Asset::from_symbol(&pair.token1.symbol, "Aerodrome");

        // TVL
        let tvl = pair.total_value_locked_usd.as_deref()
            .or(pair.reserve_usd.as_deref())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        if tvl <= 0.0 {
            return None;
        }

        // Pool type
        let is_stable = pair.is_stable.or(pair.stable).unwrap_or(false);
        let pool_type = if is_stable {
            PoolType::Standard
        } else {
            PoolType::ConcentratedLiquidity
        };

        // Fee tier: try fee field, then feeTier
        let fee_rate_bps = pair.fee.as_deref()
            .or(pair.fee_tier.as_deref())
            .and_then(|s| {
                let val: f64 = s.parse().ok()?;
                if val > 1.0 {
                    // Raw bps or Uni-style (100=0.01%, 3000=0.30%)
                    Some((val / 100.0).round() as u32)
                } else if val > 0.0 {
                    // Decimal percentage (0.003 = 0.3%)
                    Some((val * 10000.0).round() as u32)
                } else {
                    None
                }
            })
            .unwrap_or(if is_stable { 5 } else { 30 });

        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        // Day data
        let day_data = if is_solidly {
            pair.pair_day_data.unwrap_or_default()
        } else {
            pair.pool_day_data.unwrap_or_default()
        };

        // 24h metrics
        let (fees_24h, volume_24h, fee_apr_24h) = if let Some(day) = day_data.first() {
            let fees: f64 = day.fees_usd.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let vol: f64 = day.volume_usd.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let day_tvl: f64 = day.reserve_usd.as_deref()
                .or(day.tvl_usd.as_deref())
                .and_then(|s| s.parse().ok())
                .unwrap_or(tvl);
            // If no feesUSD in day data, derive from volume
            let actual_fees = if fees > 0.0 { fees } else { vol * fee_rate_bps as f64 / 10000.0 };
            let apr = if day_tvl > 0.0 { (actual_fees * 365.0 / day_tvl) * 100.0 } else { 0.0 };
            (actual_fees, vol, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        // 7-day averages — extrapolate to 7 days when fewer days of data are available
        let (fees_7d, volume_7d, fee_apr_7d) = if !day_data.is_empty() {
            let days = day_data.len() as f64;
            let total_fees: f64 = day_data.iter()
                .map(|d| {
                    let f = d.fees_usd.as_deref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    if f > 0.0 { f } else {
                        let v = d.volume_usd.as_deref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        v * fee_rate_bps as f64 / 10000.0
                    }
                })
                .sum();
            let total_volume: f64 = day_data.iter()
                .map(|d| d.volume_usd.as_deref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0))
                .sum();
            let avg_tvl: f64 = day_data.iter()
                .map(|d| {
                    d.reserve_usd.as_deref()
                        .or(d.tvl_usd.as_deref())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0)
                })
                .sum::<f64>() / days;
            let daily_avg_fees = total_fees / days;
            let daily_avg_volume = total_volume / days;
            let fees_7d = daily_avg_fees * 7.0;
            let volume_7d = daily_avg_volume * 7.0;
            let effective_tvl = if avg_tvl > 0.0 { avg_tvl } else { tvl };
            let apr = if effective_tvl > 0.0 { (daily_avg_fees * 365.0 / effective_tvl) * 100.0 } else { 0.0 };
            (fees_7d, volume_7d, apr)
        } else {
            (0.0, 0.0, 0.0)
        };

        if fee_apr_24h > 10000.0 || fee_apr_7d > 10000.0 {
            return None;
        }

        Some(PoolRate {
            protocol: Protocol::Aerodrome,
            chain: Chain::Base,
            token0,
            token1,
            pool_type,
            fee_tier,
            fee_rate_bps,
            tvl_usd: tvl,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr: 0.0, // TODO: Calculate from gauge emissions when AERO price available
            pool_address: pair.id.clone(),
            pool_id: Some(pair.id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(pool_address: &str) -> String {
        format!("https://aerodrome.finance/pool/{}", pool_address)
    }
}

#[async_trait]
impl PoolIndexer for AerodromeIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Aerodrome
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Base]
    }

    async fn fetch_pools(&self, chain: &Chain) -> Result<Vec<PoolRate>> {
        if *chain != Chain::Base {
            return Ok(vec![]);
        }
        self.fetch_pools().await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.pool_address)
    }
}
