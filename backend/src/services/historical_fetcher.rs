use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use crate::models::{Protocol, Chain, RateResult};

/// Historical data point from external sources (TheGraph, APIs)
#[derive(Debug, Clone)]
pub struct HistoricalDataPoint {
    pub date: DateTime<Utc>,
    pub supply_apy: f64,
    pub borrow_apr: f64,
    pub total_liquidity: u64,
    pub available_liquidity: u64,
    pub utilization_rate: u32,
}

/// Fetches real historical data from protocol-specific sources
pub struct HistoricalFetcher {
    client: reqwest::Client,
    graph_api_key: Option<String>,
}

impl HistoricalFetcher {
    pub fn new(graph_api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            graph_api_key,
        }
    }
    
    /// Fetch historical data for a specific vault/pool
    pub async fn fetch_historical_data(
        &self,
        protocol: &Protocol,
        chain: &Chain,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        match protocol {
            Protocol::Aave => self.fetch_aave_historical(chain, rate, start_date, end_date).await,
            Protocol::Morpho => self.fetch_morpho_historical(chain, rate, start_date, end_date).await,
            Protocol::SparkLend => self.fetch_sparklend_historical(chain, rate, start_date, end_date).await,
            Protocol::Lido => self.fetch_lido_historical(chain, start_date, end_date).await,
            Protocol::Marinade => self.fetch_marinade_historical(start_date, end_date).await,
            Protocol::Kamino => self.fetch_kamino_historical(rate, start_date, end_date).await,
            Protocol::Fluid => self.fetch_fluid_historical(rate, start_date, end_date).await,
            _ => {
                tracing::warn!("Historical fetcher not implemented for {:?}", protocol);
                Ok(vec![]) // Return empty, will skip
            }
        }
    }
    
    /// Aave: TheGraph subgraph (V3 markets)
    async fn fetch_aave_historical(
        &self,
        chain: &Chain,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        // Official Aave v3 subgraph deployment IDs from https://github.com/aave/protocol-subgraphs
        let subgraph_id = match chain {
            Chain::Ethereum => "Cd2gEDVeqnjBn1hSeqFMitw8Q1iiyV9FYUZkLNRcL87g",
            Chain::Arbitrum => "DLuE98kEb5pQNXAcKFQGQgfSQ57Xdou4jnVbAEqMfy3B",
            Chain::Base     => "GQFbb95cE6d8mV989mL5figjaGaKCQB3xqYrr1bRyXqF",
            Chain::Polygon  => "Co2URyXjnxaw8WqxKyVHdirq9Ahhm5vcTs4dMedAq211",
            Chain::Optimism => "DSfLz8oQBUeU5atALgUFQKMTSYV9mZAVYp4noLSXAfvb",
            Chain::Avalanche=> "2h9woxy8RTjHu1HJsCEnmzpPHFArU33avmUh4f71JpVn",
            _ => return Ok(vec![]),
        };
        
        // Require a Graph Studio API key — the deprecated hosted service is gone.
        let api_key = self.graph_api_key.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "THE_GRAPH_API_KEY not configured. \
                 Aave historical data requires a Graph Studio API key. \
                 Get one at https://thegraph.com/studio/ and set THE_GRAPH_API_KEY env var."
            )
        })?;

        let endpoint_url = format!(
            "https://gateway.thegraph.com/api/{}/subgraphs/id/{}",
            api_key, subgraph_id
        );
        
        // Extract reserve address from URL (if available)
        let reserve_address = self.extract_address_from_url(&rate.url);
        
        let query = format!(r#"
        {{
          reserves(where: {{underlyingAsset: "{}"}}) {{
            id
            symbol
            paramsHistory(
              where: {{timestamp_gte: {}, timestamp_lte: {}}}
              orderBy: timestamp
              orderDirection: asc
              first: 1000
            ) {{
              timestamp
              liquidityRate
              variableBorrowRate
              totalLiquidity
              availableLiquidity
              utilizationRate
            }}
          }}
        }}
        "#, 
            reserve_address.unwrap_or("".to_string()),
            start_date.timestamp(),
            end_date.timestamp()
        );
        
        tracing::debug!("🌐 Aave GraphQL endpoint: {}", endpoint_url);
        
        let response = self.client
            .post(&endpoint_url)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .context("Failed to query Aave subgraph")?;
        
        let status = response.status();
        tracing::debug!("📡 Aave GraphQL Response: {}", status);
        
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Failed to read error".to_string());
            tracing::warn!("⚠️ Aave GraphQL error {}: {}", status, error_body);
            return Ok(vec![]);
        }
        
        // Read the raw body so we can log it before deserialization
        let raw_body = response.text().await.context("Failed to read Aave subgraph response body")?;
        tracing::debug!("📦 Aave raw response (first 500 chars): {}", &raw_body[..raw_body.len().min(500)]);
        
        let data: AaveSubgraphResponse = serde_json::from_str(&raw_body)
            .context("Failed to deserialize Aave subgraph response")?;
        
        if let Some(reserves) = data.data {
            if let Some(reserve) = reserves.reserves.first() {
                let points: Vec<HistoricalDataPoint> = reserve.params_history.iter().map(|h| {
                    HistoricalDataPoint {
                        date: DateTime::from_timestamp(h.timestamp, 0).unwrap_or(Utc::now()),
                        supply_apy: h.liquidity_rate.parse::<f64>().unwrap_or(0.0) / 1e27 * 100.0, // Ray (1e27) to %
                        borrow_apr: h.variable_borrow_rate.parse::<f64>().unwrap_or(0.0) / 1e27 * 100.0,
                        total_liquidity: h.total_liquidity.parse::<u64>().unwrap_or(0),
                        available_liquidity: h.available_liquidity.parse::<u64>().unwrap_or(0),
                        utilization_rate: (h.utilization_rate.parse::<f64>().unwrap_or(0.0) * 100.0) as u32,
                    }
                }).collect();
                
                tracing::info!("✅ Found {} Aave historical data points", points.len());
                return Ok(points);
            }
        }
        
        tracing::warn!("⚠️ No Aave historical data found for reserve");
        Ok(vec![])
    }
    
    /// Morpho: Official API (has historical endpoints)
    async fn fetch_morpho_historical(
        &self,
        chain: &Chain,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        tracing::info!("Fetching Morpho {:?} historical data from {} to {}...", chain, start_date, end_date);
        
        // For Morpho, extract the vault address (40-char Ethereum address) from the URL
        // Example: https://app.morpho.org/ethereum/vault/0xfeaC08ffA38d95ec5Ed7C46c933C8891a44C5F26
        let vault_address = self.extract_address_from_url(&rate.url);
        
        if vault_address.is_none() {
            tracing::warn!("⚠️ Could not extract Morpho vault address from URL: {}", rate.url);
            return Ok(vec![]);
        }
        
        let address = vault_address.unwrap();
        tracing::debug!("📌 Extracted Morpho vault address: {}", address);
        
        // Use GraphQL API to fetch historical data
        let chain_id = self.chain_to_morpho_chain_id(chain);
        let graphql_url = "https://api.morpho.org/graphql";
        
        let query = serde_json::json!({
            "query": r#"
                query GetVaultHistory($address: String!, $chainId: Int!) {
                    vaultByAddress(address: $address, chainId: $chainId) {
                        address
                        name
                        historicalState {
                            netApy {
                                x
                                y
                            }
                            totalAssets {
                                x
                                y
                            }
                        }
                    }
                }
            "#,
            "variables": {
                "address": address,
                "chainId": chain_id
            }
        });
        
        tracing::debug!("🌐 Morpho GraphQL query: {}", serde_json::to_string_pretty(&query)?);
        
        let response = self.client
            .post(graphql_url)
            .json(&query)
            .send()
            .await
            .context("Failed to send GraphQL request to Morpho API")?;
        
        let status = response.status();
        tracing::debug!("📡 Morpho API Response: {}", status);
        
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
            tracing::warn!("⚠️ Morpho API error {}: {}", status, error_body);
            return Ok(vec![]);
        }
        
        let text = response.text().await?;
        tracing::debug!("📦 Morpho Response size: {} bytes", text.len());
        
        let graphql_response: MorphoHistoricalGraphQLResponse = serde_json::from_str(&text)
            .context("Failed to parse Morpho GraphQL response")?;
        
        if let Some(vault) = graphql_response.data.and_then(|d| d.vault_by_address) {
            if let Some(historical_state) = vault.historical_state {
                // Convert historical state to data points
                let mut points = Vec::new();
                
                // historicalState contains arrays of {x: timestamp, y: value}
                if let Some(net_apy) = historical_state.net_apy {
                    for data_point in net_apy {
                        // x is Unix timestamp in milliseconds
                        let timestamp = DateTime::from_timestamp(data_point.x / 1000, 0)
                            .unwrap_or_else(|| Utc::now());
                        
                        // Filter by date range
                        if timestamp >= start_date && timestamp <= end_date {
                            // y is APY as decimal (e.g., 0.05 for 5%)
                            let apy = data_point.y * 100.0;
                            
                            points.push(HistoricalDataPoint {
                                date: timestamp,
                                supply_apy: apy,
                                borrow_apr: 0.0,
                                total_liquidity: 0, // Not available in this response
                                available_liquidity: 0,
                                utilization_rate: 0,
                            });
                        }
                    }
                }
                
                tracing::info!("✅ Found {} Morpho historical data points", points.len());
                return Ok(points);
            }
        }
        
        tracing::warn!("⚠️ No historical data found for vault {}", address);
        Ok(vec![])
    }
    
    /// Kamino: Use their official API (has historical endpoints)
    async fn fetch_kamino_historical(
        &self,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        // Kamino API has /kamino-market/{market}/history endpoint
        // Need to parse market address from rate data
        
        tracing::debug!("Kamino historical data fetch not yet implemented");
        Ok(vec![])
    }
    
    /// Fluid: Official API (check if they expose historical)
    async fn fetch_fluid_historical(
        &self,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        tracing::debug!("Fluid historical data fetch not yet implemented");
        Ok(vec![])
    }
    
    /// SparkLend: Uses Aave V3 API (MakerDAO fork)
    async fn fetch_sparklend_historical(
        &self,
        chain: &Chain,
        rate: &RateResult,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        // SparkLend uses Aave v3 GraphQL API
        let (chain_id, market_address) = match chain {
            Chain::Ethereum => (1, "0xC13e21B648A5Ee794902342038FF3aDAB66BE987"),
            _ => return Ok(vec![]),
        };
        
        tracing::info!("Fetching SparkLend historical data for {:?}...", chain);
        
        // Extract reserve address from URL
        let reserve_address = self.extract_address_from_url(&rate.url);
        if reserve_address.is_none() {
            return Ok(vec![]);
        }
        
        let query = format!(r#"
        {{
          market(request: {{chainId: {}, address: "{}"}}) {{
            reserves(where: {{underlyingAsset: "{}"}}) {{
              underlyingToken {{
                symbol
                address
              }}
              paramsHistory(
                where: {{timestamp_gte: {}, timestamp_lte: {}}}
                orderBy: timestamp
                orderDirection: asc
                first: 1000
              ) {{
                timestamp
                liquidityRate
                variableBorrowRate
                totalLiquidity
                availableLiquidity
                utilizationRate
              }}
            }}
          }}
        }}
        "#,
            chain_id,
            market_address,
            reserve_address.unwrap(),
            start_date.timestamp(),
            end_date.timestamp()
        );
        
        let response = self.client
            .post("https://api.v3.aave.com/graphql")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .context("Failed to query SparkLend via Aave API")?;
        
        if !response.status().is_success() {
            tracing::warn!("SparkLend API returned status: {}", response.status());
            return Ok(vec![]);
        }
        
        let data: SparkLendResponse = response.json().await?;
        
        if let Some(market) = data.data {
            if let Some(reserves) = market.market {
                if let Some(reserve) = reserves.reserves.first() {
                    let points: Vec<HistoricalDataPoint> = reserve.params_history.iter().map(|h| {
                        HistoricalDataPoint {
                            date: DateTime::from_timestamp(h.timestamp.parse().unwrap_or(0), 0).unwrap_or(Utc::now()),
                            supply_apy: h.liquidity_rate.parse::<f64>().unwrap_or(0.0) / 1e27 * 100.0, // Ray (1e27) to %
                            borrow_apr: h.variable_borrow_rate.parse::<f64>().unwrap_or(0.0) / 1e27 * 100.0,
                            total_liquidity: h.total_liquidity.parse::<u64>().unwrap_or(0),
                            available_liquidity: h.available_liquidity.parse::<u64>().unwrap_or(0),
                            utilization_rate: (h.utilization_rate.parse::<f64>().unwrap_or(0.0) * 100.0) as u32,
                        }
                    }).collect();
                    
                    tracing::info!("Found {} SparkLend historical data points", points.len());
                    return Ok(points);
                }
            }
        }
        
        Ok(vec![])
    }
    
    /// Lido: Use DeFi Llama historical chart data
    async fn fetch_lido_historical(
        &self,
        chain: &Chain,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        // Lido official API doesn't have full historical endpoint
        // Use DeFi Llama yields API for historical APY
        
        let pool_id = match chain {
            Chain::Ethereum => "747c1d2a-c668-4682-b9f9-296708a3dd90", // stETH pool (CORRECT)
            // Note: Lido Solana stSOL pool not found in DeFi Llama
            _ => {
                tracing::warn!("⚠️ Lido {:?} not supported in DeFi Llama", chain);
                return Ok(vec![]);
            }
        };
        
        tracing::info!("Fetching Lido {:?} historical data from {} to {}...", chain, start_date, end_date);
        
        let url = format!("https://yields.llama.fi/chart/{}", pool_id);
        tracing::debug!("🌐 Lido API URL: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Lido historical from DeFi Llama")?;
        
        let status = response.status();
        tracing::debug!("📡 Lido API Response: {}", status);
        
        if !status.is_success() {
            tracing::warn!("⚠️ DeFi Llama API returned status: {}", status);
            return Ok(vec![]);
        }
        
        let text = response.text().await?;
        tracing::debug!("📦 Lido Response size: {} bytes", text.len());
        
        let data: DefiLlamaChartResponse = serde_json::from_str(&text)
            .context("Failed to parse Lido JSON response")?;
        
        let raw_count = data.data.len();
        tracing::debug!("📊 Lido raw data points: {}", raw_count);
        
        // Filter by date range
        let points: Vec<HistoricalDataPoint> = data.data
            .into_iter()
            .filter(|p| {
                let point_date = DateTime::parse_from_rfc3339(&p.timestamp)
                    .ok()
                    .map(|d| d.with_timezone(&Utc));
                if let Some(date) = point_date {
                    date >= start_date && date <= end_date
                } else {
                    false
                }
            })
            .map(|p| {
                let date = DateTime::parse_from_rfc3339(&p.timestamp)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or(Utc::now());
                    
                HistoricalDataPoint {
                    date,
                    supply_apy: p.apy,
                    borrow_apr: 0.0,
                    total_liquidity: p.tvl_usd as u64,
                    available_liquidity: p.tvl_usd as u64,
                    utilization_rate: 100, // Staking is 100% utilized
                }
            })
            .collect();
        
        tracing::info!("✅ Found {} Lido historical data points (filtered from {})", points.len(), raw_count);
        Ok(points)
    }
    
    /// Marinade: Try to use extended APY endpoint
    async fn fetch_marinade_historical(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<HistoricalDataPoint>> {
        tracing::info!("Fetching Marinade historical data from {} to {}...", start_date, end_date);
        
        // Marinade API may not have historical endpoint
        // Try DeFi Llama as alternative
        let pool_id = "b3f93865-5ec8-4662-90a0-11808e0aa2bd"; // mSOL pool ID (CORRECT)
        let url = format!("https://yields.llama.fi/chart/{}", pool_id);
        
        tracing::debug!("🌐 Marinade API URL: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await;
        
        if let Ok(resp) = response {
            let status = resp.status();
            tracing::debug!("📡 Marinade API Response: {}", status);
            
            if status.is_success() {
                let text = resp.text().await?;
                tracing::debug!("📦 Marinade Response size: {} bytes", text.len());
                
                if let Ok(data) = serde_json::from_str::<DefiLlamaChartResponse>(&text) {
                    let raw_count = data.data.len();
                    tracing::debug!("📊 Marinade raw data points: {}", raw_count);
                    let points: Vec<HistoricalDataPoint> = data.data
                        .into_iter()
                        .filter(|p| {
                            let point_date = DateTime::parse_from_rfc3339(&p.timestamp)
                                .ok()
                                .map(|d| d.with_timezone(&Utc));
                            if let Some(date) = point_date {
                                date >= start_date && date <= end_date
                            } else {
                                false
                            }
                        })
                        .map(|p| {
                            let date = DateTime::parse_from_rfc3339(&p.timestamp)
                                .ok()
                                .map(|d| d.with_timezone(&Utc))
                                .unwrap_or(Utc::now());
                                
                            HistoricalDataPoint {
                                date,
                                supply_apy: p.apy,
                                borrow_apr: 0.0,
                                total_liquidity: p.tvl_usd as u64,
                                available_liquidity: p.tvl_usd as u64,
                                utilization_rate: 100,
                            }
                        })
                        .collect();
                    
                    tracing::info!("✅ Found {} Marinade historical data points (filtered from {})", points.len(), raw_count);
                    return Ok(points);
                } else {
                    tracing::error!("❌ Failed to parse Marinade JSON response");
                }
            } else {
                tracing::warn!("⚠️ Marinade API returned status: {}", status);
            }
        } else {
            tracing::error!("❌ Failed to send request to Marinade API");
        }
        
        tracing::warn!("Could not fetch Marinade historical data");
        Ok(vec![])
    }
    
    // Helper methods
    
    fn extract_address_from_url(&self, url: &str) -> Option<String> {
        // Extract Ethereum address (0x followed by EXACTLY 40 hex chars).
        // Note: the `regex` crate does NOT support lookaheads, so we match
        // 0x + 40 hex chars and then manually verify the next char is not hex,
        // catching cases where we might otherwise match the first 40 chars of
        // a 64-char tx hash.
        let re = Regex::new(r"(?i)0x[0-9a-f]{40}").ok()?;
        
        for capture in re.find_iter(url) {
            let end = capture.end();
            // Ensure the next character (if any) is NOT a hex char — avoids
            // matching the prefix of a 64-char transaction hash.
            let next_is_hex = url[end..].chars().next()
                .map(|c| c.is_ascii_hexdigit())
                .unwrap_or(false);
            if !next_is_hex {
                let address = capture.as_str().to_lowercase();
                tracing::debug!("📌 Extracted Ethereum address: {} (from: {})", address, url);
                return Some(address);
            }
        }
        
        tracing::warn!("⚠️ No valid Ethereum address found in URL: {}", url);
        None
    }
    
    fn chain_to_morpho_chain_id(&self, chain: &Chain) -> i32 {
        match chain {
            Chain::Ethereum => 1,
            Chain::Base => 8453,
            Chain::Arbitrum => 42161,
            Chain::Optimism => 10,
            Chain::Polygon => 137,
            _ => 1,
        }
    }
}

// Response types for external APIs

#[derive(Debug, Deserialize)]
struct AaveSubgraphResponse {
    data: Option<AaveSubgraphData>,
}

#[derive(Debug, Deserialize)]
struct AaveSubgraphData {
    reserves: Vec<AaveReserve>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AaveReserve {
    id: String,
    symbol: String,
    params_history: Vec<AaveParamsHistory>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AaveParamsHistory {
    timestamp: i64,
    liquidity_rate: String,
    variable_borrow_rate: String,
    total_liquidity: String,
    available_liquidity: String,
    utilization_rate: String,
}

#[derive(Debug, Deserialize)]
struct MorphoHistoricalResponse {
    history: Vec<MorphoHistoryPoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphoHistoryPoint {
    date: DateTime<Utc>,
    net_apy: f64,
    total_assets: u64,
    available_assets: u64,
}

// Morpho GraphQL response types for historical data
#[derive(Debug, Deserialize)]
struct MorphoHistoricalGraphQLResponse {
    data: Option<MorphoHistoricalGraphQLData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphoHistoricalGraphQLData {
    vault_by_address: Option<MorphoVaultHistorical>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphoVaultHistorical {
    address: String,
    name: String,
    historical_state: Option<MorphoHistoricalState>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphoHistoricalState {
    net_apy: Option<Vec<MorphoDataPoint>>,
    total_assets: Option<Vec<MorphoDataPoint>>,
}

#[derive(Debug, Deserialize)]
struct MorphoDataPoint {
    x: i64, // Unix timestamp in milliseconds
    y: f64, // Value (APY as decimal, or total assets)
}

// SparkLend response types (uses Aave API structure)
#[derive(Debug, Deserialize)]
struct SparkLendResponse {
    data: Option<SparkLendData>,
}

#[derive(Debug, Deserialize)]
struct SparkLendData {
    market: Option<SparkLendMarket>,
}

#[derive(Debug, Deserialize)]
struct SparkLendMarket {
    reserves: Vec<SparkLendReserve>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SparkLendReserve {
    underlying_token: SparkLendToken,
    params_history: Vec<SparkLendParamsHistory>,
}

#[derive(Debug, Deserialize)]
struct SparkLendToken {
    symbol: String,
    address: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SparkLendParamsHistory {
    timestamp: String,
    liquidity_rate: String,
    variable_borrow_rate: String,
    total_liquidity: String,
    available_liquidity: String,
    utilization_rate: String,
}

// DeFi Llama chart response
#[derive(Debug, Deserialize)]
struct DefiLlamaChartResponse {
    data: Vec<DefiLlamaChartPoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefiLlamaChartPoint {
    timestamp: String,
    apy: f64,
    #[serde(rename = "tvlUsd")]
    tvl_usd: f64,
}

#[cfg(test)]
#[path = "historical_fetcher_test.rs"]
mod tests;

