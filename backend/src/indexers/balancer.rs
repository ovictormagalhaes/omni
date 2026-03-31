use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::time::Duration;

use super::PoolIndexer;
use crate::models::{Asset, Chain, FeeTier, PoolRate, PoolType, Protocol};

// ============================================================================
// Balancer V2 - Native GraphQL API Integration
// ============================================================================
// Multi-chain DEX with weighted pools.
// API: https://api-v3.balancer.fi/ (GraphQL)
// Supported chains: Ethereum, Arbitrum, Base, Polygon, Optimism, Avalanche
// ============================================================================

const BALANCER_API_URL: &str = "https://api-v3.balancer.fi/";

// GraphQL query to fetch pools ordered by TVL with APR data
const POOLS_QUERY: &str = r#"
query GetPools($chains: [GqlChain!]!) {
  poolGetPools(
    first: 100,
    orderBy: totalLiquidity,
    orderDirection: desc,
    where: { chainIn: $chains, minTvl: 10000 }
  ) {
    id
    name
    chain
    type
    dynamicData {
      totalLiquidity
      volume24h
      fees24h
      aprItems {
        apr
        type
      }
    }
    poolTokens {
      address
      symbol
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLData {
    pool_get_pools: Vec<BalancerPool>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BalancerPool {
    id: String,
    #[allow(dead_code)]
    name: Option<String>,
    chain: String,
    #[serde(rename = "type")]
    pool_type: Option<String>,
    dynamic_data: Option<PoolDynamicData>,
    pool_tokens: Option<Vec<PoolToken>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolDynamicData {
    total_liquidity: String,
    volume24h: String,
    fees24h: String,
    apr_items: Vec<AprItem>,
}

#[derive(Debug, Deserialize)]
struct AprItem {
    apr: f64,
    #[serde(rename = "type")]
    apr_type: String,
}

#[derive(Debug, Deserialize)]
struct PoolToken {
    #[allow(dead_code)]
    address: String,
    symbol: String,
}

#[derive(Clone)]
pub struct BalancerIndexer {
    client: reqwest::Client,
}

impl Default for BalancerIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl BalancerIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_pools(&self) -> Result<Vec<PoolRate>> {
        tracing::info!("[Balancer] Fetching pools from Balancer GraphQL API");

        let chains = [
            (Chain::Ethereum, "MAINNET"),
            (Chain::Arbitrum, "ARBITRUM"),
            (Chain::Base, "BASE"),
            (Chain::Polygon, "POLYGON"),
            (Chain::Optimism, "OPTIMISM"),
            (Chain::Avalanche, "AVALANCHE"),
        ];

        let chain_names: Vec<&str> = chains.iter().map(|(_, name)| *name).collect();

        let query_body = serde_json::json!({
            "query": POOLS_QUERY,
            "variables": {
                "chains": chain_names,
            }
        });

        let response = self
            .client
            .post(BALANCER_API_URL)
            .timeout(Duration::from_secs(30))
            .header("Content-Type", "application/json")
            .json(&query_body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[Balancer] Failed to fetch from API: {}", e);
                return Ok(vec![]);
            }
        };

        if !response.status().is_success() {
            tracing::warn!("[Balancer] API returned status: {}", response.status());
            return Ok(vec![]);
        }

        let gql_response: GraphQLResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("[Balancer] Failed to parse response: {}", e);
                return Ok(vec![]);
            }
        };

        if let Some(errors) = &gql_response.errors {
            for err in errors {
                tracing::warn!("[Balancer] GraphQL error: {}", err.message);
            }
            if gql_response.data.is_none() {
                return Ok(vec![]);
            }
        }

        let pools = match gql_response.data {
            Some(data) => data.pool_get_pools,
            None => {
                tracing::warn!("[Balancer] No data in GraphQL response");
                return Ok(vec![]);
            }
        };

        tracing::info!("[Balancer] Received {} pools from API", pools.len());

        let rates: Vec<PoolRate> = pools
            .into_iter()
            .filter_map(|p| self.parse_pool(p))
            .collect();

        tracing::info!("[Balancer] Parsed {} pools after filtering", rates.len());
        Ok(rates)
    }

    fn parse_pool(&self, pool: BalancerPool) -> Option<PoolRate> {
        let chain = parse_api_chain(&pool.chain)?;

        let tokens = pool.pool_tokens.as_ref()?;
        if tokens.len() < 2 {
            return None;
        }

        let token0_str = clean_token_symbol(&tokens[0].symbol);
        let token1_str = clean_token_symbol(&tokens[1].symbol);

        if token0_str.is_empty() || token1_str.is_empty() {
            return None;
        }

        let token0 = Asset::from_symbol(&token0_str, "Balancer");
        let token1 = Asset::from_symbol(&token1_str, "Balancer");

        if pool.id.is_empty() {
            return None;
        }

        let dynamic = pool.dynamic_data?;

        let tvl_usd: f64 = dynamic.total_liquidity.parse().unwrap_or(0.0);
        if tvl_usd < 10000.0 {
            return None;
        }

        let volume_24h: f64 = dynamic.volume24h.parse().unwrap_or(0.0);
        let fees_24h: f64 = dynamic.fees24h.parse().unwrap_or(0.0);

        // Calculate fee_rate_bps from actual fees/volume ratio
        let fee_rate_bps: u32 = if volume_24h > 0.0 {
            ((fees_24h / volume_24h) * 10000.0).round() as u32
        } else {
            30 // Balancer typical default
        };
        let fee_tier = FeeTier::from_bps(fee_rate_bps);

        // Separate swap fee APR from reward APR
        let mut fee_apr_24h = 0.0_f64;
        let mut rewards_apr = 0.0_f64;

        for item in &dynamic.apr_items {
            // APR values are decimal (0.005 = 0.5%), convert to percentage
            let apr_pct = item.apr * 100.0;
            match item.apr_type.as_str() {
                "SWAP_FEE_24H" | "SWAP_FEE_7D" => fee_apr_24h += apr_pct,
                _ => rewards_apr += apr_pct,
            }
        }

        // Sanity check
        if fee_apr_24h > 10000.0 || rewards_apr > 10000.0 {
            return None;
        }

        let fees_7d = fees_24h * 7.0; // estimate
        let volume_7d = volume_24h * 7.0; // estimate

        let fee_apr_7d = if tvl_usd > 0.0 && fees_7d > 0.0 {
            let daily_avg_fees = fees_7d / 7.0;
            (daily_avg_fees * 365.0 / tvl_usd) * 100.0
        } else {
            0.0
        };

        let pool_type = match pool.pool_type.as_deref() {
            Some("CONCENTRATED") => PoolType::ConcentratedLiquidity,
            _ => PoolType::Standard,
        };

        Some(PoolRate {
            protocol: Protocol::Balancer,
            chain,
            token0,
            token1,
            pool_type,
            fee_tier,
            fee_rate_bps,
            tvl_usd,
            volume_24h_usd: volume_24h,
            volume_7d_usd: volume_7d,
            fees_24h_usd: fees_24h,
            fees_7d_usd: fees_7d,
            fee_apr_24h,
            fee_apr_7d,
            rewards_apr,
            pool_address: pool.id.clone(),
            pool_id: Some(pool.id),
            active: true,
            timestamp: Utc::now(),
        })
    }

    pub fn get_pool_url(chain: &Chain) -> String {
        let chain_slug = match chain {
            Chain::Ethereum => "ethereum",
            Chain::Arbitrum => "arbitrum",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Optimism => "optimism",
            Chain::Avalanche => "avalanche",
            _ => "ethereum",
        };
        format!("https://balancer.fi/pools?network={}", chain_slug)
    }
}

#[async_trait]
impl PoolIndexer for BalancerIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Balancer
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
            Chain::Avalanche,
        ]
    }

    async fn fetch_pools(&self, _chain: &Chain) -> Result<Vec<PoolRate>> {
        self.fetch_pools().await
    }

    fn pool_url(&self, pool: &PoolRate) -> String {
        Self::get_pool_url(&pool.chain)
    }
}

fn parse_api_chain(chain: &str) -> Option<Chain> {
    match chain {
        "MAINNET" => Some(Chain::Ethereum),
        "ARBITRUM" => Some(Chain::Arbitrum),
        "BASE" => Some(Chain::Base),
        "POLYGON" => Some(Chain::Polygon),
        "OPTIMISM" => Some(Chain::Optimism),
        "AVALANCHE" => Some(Chain::Avalanche),
        _ => None,
    }
}

/// Strip wrapper token prefixes (e.g., "waEthUSDT" → "USDT", "waArbGHO" → "GHO")
fn clean_token_symbol(symbol: &str) -> String {
    let s = symbol.to_uppercase();
    // Common Balancer wrapped Aave token patterns: waEthX, waArbX, waBaseX, etc.
    if s.starts_with("WA") && s.len() > 4 {
        // Try to extract the underlying token (last part after chain prefix)
        let known_prefixes = ["WAETH", "WAARB", "WABASE", "WAPOL", "WAOPT", "WAAVAX"];
        for prefix in known_prefixes {
            if let Some(stripped) = s.strip_prefix(prefix) {
                return stripped.to_string();
            }
        }
    }
    s
}
