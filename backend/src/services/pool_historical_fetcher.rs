use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use crate::indexers::uniswap_v3::SUBGRAPH_IDS;
use crate::models::{Chain, Protocol};

/// Historical data point for a liquidity pool
#[derive(Debug, Clone)]
pub struct PoolHistoricalDataPoint {
    pub date: DateTime<Utc>,
    pub tvl_usd: f64,
    pub volume_usd: f64,
    pub fees_usd: f64,
}

/// Fetches real historical data for liquidity pools from protocol-specific sources
pub struct PoolHistoricalFetcher {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

// GraphQL response structures for poolDayData backfill
// Uses serde aliases to handle both `volumeUSD` and `volumeUsd` field naming
#[derive(Debug, Deserialize)]
struct PoolDayDataResponse {
    data: Option<PoolDayDataWrapper>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolDayDataWrapper {
    pool_day_datas: Vec<PoolDayDataEntry>,
}

#[derive(Debug, Deserialize)]
struct PoolDayDataEntry {
    date: i64,
    #[serde(alias = "volumeUSD", alias = "volumeUsd", alias = "volume_usd")]
    volume_usd: String,
    #[serde(alias = "tvlUSD", alias = "tvlUsd", alias = "tvl_usd")]
    tvl_usd: String,
    #[serde(alias = "feesUSD", alias = "feesUsd", alias = "fees_usd")]
    fees_usd: String,
    /// Pool address — only present in batch queries (via the `pool { id }` field)
    #[serde(default)]
    #[allow(dead_code)]
    pool_id: Option<String>,
}

/// Wrapper for batch query: pool_id is nested as `pool { id }`
#[derive(Debug, Deserialize)]
struct PoolDayDataEntryBatch {
    date: i64,
    #[serde(alias = "volumeUSD", alias = "volumeUsd", alias = "volume_usd")]
    volume_usd: String,
    #[serde(alias = "tvlUSD", alias = "tvlUsd", alias = "tvl_usd")]
    tvl_usd: String,
    #[serde(alias = "feesUSD", alias = "feesUsd", alias = "fees_usd")]
    fees_usd: String,
    pool: PoolIdRef,
}

#[derive(Debug, Deserialize)]
struct PoolIdRef {
    id: String,
}

#[derive(Debug, Deserialize)]
struct BatchPoolDayDataResponse {
    data: Option<BatchPoolDayDataWrapper>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchPoolDayDataWrapper {
    pool_day_datas: Vec<PoolDayDataEntryBatch>,
}

impl PoolHistoricalFetcher {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            graph_api_key,
        }
    }

    /// Fetch historical data for a specific liquidity pool
    pub async fn fetch_pool_historical_data(
        &self,
        protocol: &Protocol,
        chain: &Chain,
        pool_address: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<PoolHistoricalDataPoint>> {
        match protocol {
            Protocol::Uniswap => {
                self.fetch_uniswap_v3_pool_history(chain, pool_address, start_date, end_date)
                    .await
            }
            _ => {
                tracing::debug!("Pool historical fetcher not implemented for {:?}", protocol);
                Ok(vec![])
            }
        }
    }

    /// Batch fetch historical data for multiple pools on the same chain in a single GraphQL query.
    /// Returns a HashMap of pool_address → Vec<PoolHistoricalDataPoint>.
    pub async fn fetch_pool_historical_batch(
        &self,
        protocol: &Protocol,
        chain: &Chain,
        pool_addresses: &[String],
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<HashMap<String, Vec<PoolHistoricalDataPoint>>> {
        if pool_addresses.is_empty() {
            return Ok(HashMap::new());
        }

        match protocol {
            Protocol::Uniswap => {
                self.fetch_uniswap_v3_batch(chain, pool_addresses, start_date, end_date)
                    .await
            }
            _ => Ok(HashMap::new()),
        }
    }

    /// Batch fetch poolDayDatas for multiple Uniswap V3 pools using `pool_in` filter.
    /// The Graph supports up to 1000 results per query, so we chunk if needed.
    async fn fetch_uniswap_v3_batch(
        &self,
        chain: &Chain,
        pool_addresses: &[String],
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<HashMap<String, Vec<PoolHistoricalDataPoint>>> {
        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[Pool Backfill] No THE_GRAPH_API_KEY configured, skipping batch");
                return Ok(HashMap::new());
            }
        };

        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Celo => "celo",
            _ => return Ok(HashMap::new()),
        };

        let subgraph_id = match SUBGRAPH_IDS.iter().find(|(slug, _)| *slug == chain_slug) {
            Some((_, id)) => *id,
            None => return Ok(HashMap::new()),
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, subgraph_id
        );

        let start_ts = start_date.timestamp();
        let end_ts = end_date.timestamp();

        let mut all_results: HashMap<String, Vec<PoolHistoricalDataPoint>> = HashMap::new();

        // The Graph returns max 1000 items. With 30 days × N pools, chunk to avoid hitting limit.
        // Safe chunk: 1000 / 30 days ≈ 33 pools per query
        let chunk_size = 30;

        for chunk in pool_addresses.chunks(chunk_size) {
            let pool_list: Vec<String> = chunk
                .iter()
                .map(|a| format!("\"{}\"", a.to_lowercase()))
                .collect();
            let pool_in = pool_list.join(", ");

            let query = serde_json::json!({
                "query": format!(
                    r#"{{
                        poolDayDatas(
                            where: {{ pool_in: [{pool_in}], date_gte: {start_ts}, date_lte: {end_ts} }}
                            orderBy: date
                            orderDirection: asc
                            first: 1000
                        ) {{
                            date
                            volumeUSD
                            tvlUSD
                            feesUSD
                            pool {{ id }}
                        }}
                    }}"#
                )
            });

            tracing::debug!(
                "[Pool Backfill] Batch fetching poolDayData for {} pools on {:?}",
                chunk.len(),
                chain
            );

            let response: BatchPoolDayDataResponse =
                match self.client.post(&url).json(&query).send().await {
                    Ok(resp) => match resp.json().await {
                        Ok(data) => data,
                        Err(e) => {
                            tracing::warn!(
                                "[Pool Backfill] Failed to parse batch response on {:?}: {}",
                                chain,
                                e
                            );
                            continue;
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            "[Pool Backfill] Batch request failed on {:?}: {}",
                            chain,
                            e
                        );
                        continue;
                    }
                };

            if let Some(data) = response.data {
                for entry in data.pool_day_datas {
                    let tvl: f64 = match entry.tvl_usd.parse() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let volume: f64 = match entry.volume_usd.parse() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let fees: f64 = match entry.fees_usd.parse() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    if tvl <= 0.0 {
                        continue;
                    }

                    let date = match DateTime::from_timestamp(entry.date, 0) {
                        Some(d) => d,
                        None => continue,
                    };

                    let pool_addr = entry.pool.id.to_lowercase();
                    all_results
                        .entry(pool_addr)
                        .or_default()
                        .push(PoolHistoricalDataPoint {
                            date,
                            tvl_usd: tvl,
                            volume_usd: volume,
                            fees_usd: fees,
                        });
                }
            }
        }

        tracing::info!(
            "[Pool Backfill] Batch: got data for {} pools on {:?} ({} total addresses)",
            all_results.len(),
            chain,
            pool_addresses.len()
        );

        Ok(all_results)
    }

    /// Fetch Uniswap V3 pool history using The Graph poolDayDatas (single pool)
    async fn fetch_uniswap_v3_pool_history(
        &self,
        chain: &Chain,
        pool_address: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<PoolHistoricalDataPoint>> {
        let api_key = match &self.graph_api_key {
            Some(key) => key.clone(),
            None => {
                tracing::warn!("[Pool Backfill] No THE_GRAPH_API_KEY configured, skipping");
                return Ok(vec![]);
            }
        };

        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Celo => "celo",
            _ => return Ok(vec![]),
        };

        let subgraph_id = match SUBGRAPH_IDS.iter().find(|(slug, _)| *slug == chain_slug) {
            Some((_, id)) => *id,
            None => return Ok(vec![]),
        };

        let url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, subgraph_id
        );

        let start_ts = start_date.timestamp();
        let end_ts = end_date.timestamp();
        let pool_addr = pool_address.to_lowercase();

        let query = serde_json::json!({
            "query": format!(
                r#"{{
                    poolDayDatas(
                        where: {{ pool: "{}", date_gte: {}, date_lte: {} }}
                        orderBy: date
                        orderDirection: asc
                        first: 1000
                    ) {{
                        date
                        volumeUSD
                        tvlUSD
                        feesUSD
                    }}
                }}"#,
                pool_addr, start_ts, end_ts
            )
        });

        tracing::debug!(
            "[Pool Backfill] Fetching poolDayData for {} on {:?} ({} to {})",
            pool_addr,
            chain,
            start_date.format("%Y-%m-%d"),
            end_date.format("%Y-%m-%d")
        );

        let response: PoolDayDataResponse = self
            .client
            .post(&url)
            .json(&query)
            .send()
            .await?
            .json()
            .await?;

        let day_datas = match response.data {
            Some(data) => data.pool_day_datas,
            None => {
                tracing::debug!("[Pool Backfill] No data returned for pool {}", pool_addr);
                return Ok(vec![]);
            }
        };

        let points: Vec<PoolHistoricalDataPoint> = day_datas
            .into_iter()
            .filter_map(|d| {
                let tvl: f64 = d.tvl_usd.parse().ok()?;
                let volume: f64 = d.volume_usd.parse().ok()?;
                let fees: f64 = d.fees_usd.parse().ok()?;

                if tvl <= 0.0 {
                    return None;
                }

                let date = DateTime::from_timestamp(d.date, 0)?;

                Some(PoolHistoricalDataPoint {
                    date,
                    tvl_usd: tvl,
                    volume_usd: volume,
                    fees_usd: fees,
                })
            })
            .collect();

        tracing::debug!(
            "[Pool Backfill] Got {} daily data points for pool {} on {:?}",
            points.len(),
            pool_addr,
            chain
        );

        Ok(points)
    }
}
