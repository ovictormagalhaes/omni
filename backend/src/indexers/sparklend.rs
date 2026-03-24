use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};

// ============================================================================
// IMPORTANT NOTE: SparkLend Data Source Issue (2026-02-23)
// ============================================================================
// SparkLend is an Aave v3 fork but is NOT indexed by Aave's GraphQL API.
// 
// - Pool Address (VERIFIED): 0xC13e21B648A5Ee794902342038FF3aDAB66BE987
// - Aave v3 API (api.v3.aave.com/graphql): Returns NULL for SparkLend markets
// - The Graph: No subgraph found (searched hosted & decentralized network)
// 
// TODO: Implement alternative data source:
//   Option 1: Direct RPC calls to SparkLend Pool contract
//   Option 2: Use DeFiLlama API (https://yields.llama.fi/pools)
//   Option 3: Deploy own subgraph for SparkLend
// 
// Current Status: DISABLED - Returns empty data
// ============================================================================

const SUPPORTED_ASSETS: &[&str] = &["USDC", "USDT", "DAI", "WETH", "WSTETH", "WBTC"];
const SPARKLEND_API_URL: &str = "https://api.v3.aave.com/graphql";

// SparkLend market addresses (uses Aave v3 infrastructure)
// NOTE: These addresses are correct but not indexed by Aave's API
const MARKET_ADDRESS_ETHEREUM: &str = "0xC13e21B648A5Ee794902342038FF3aDAB66BE987";
const MARKET_ADDRESS_GNOSIS: &str = "0x2Dae5307c5E3FD1CF5A72Cb6F698f915860607e0";

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<ResponseData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    market: Market,
}

#[derive(Debug, Deserialize)]
struct Market {
    reserves: Vec<ReserveData>,
}

#[derive(Debug, Deserialize)]
struct ReserveData {
    #[serde(rename = "underlyingToken")]
    underlying_token: Token,
    #[serde(rename = "supplyInfo")]
    supply_info: Option<SupplyInfo>,
    #[serde(rename = "borrowInfo")]
    borrow_info: Option<BorrowInfo>,
    size: TokenAmount,
}

#[derive(Debug, Deserialize)]
struct Token {
    symbol: String,
    address: String,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    usd: String,
}

#[derive(Debug, Deserialize)]
struct SupplyInfo {
    apy: DecimalValue,
}

#[derive(Debug, Deserialize)]
struct BorrowInfo {
    apy: DecimalValue,
    #[serde(rename = "utilizationRate")]
    utilization_rate: Option<PercentValue>,
}

#[derive(Debug, Deserialize)]
struct DecimalValue {
    value: String,
}

#[derive(Debug, Deserialize)]
struct PercentValue {
    value: String,
}

#[derive(Debug, Clone)]
pub struct SparkLendIndexer {
    pub client: reqwest::Client,
}

impl SparkLendIndexer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        tracing::debug!("Fetching SparkLend rates for chain: {:?}", chain);

        let (chain_id, market_address) = match chain {
            Chain::Ethereum => (1, MARKET_ADDRESS_ETHEREUM),
            _ => {
                tracing::debug!("SparkLend doesn't support chain {:?}, skipping", chain);
                return Ok(vec![]);
            },
        };

        let query = json!({
            "query": format!(r#"
                query Market {{
                    market(request: {{ chainId: {}, address: "{}" }}) {{
                        reserves {{
                            underlyingToken {{
                                symbol
                                address
                            }}
                            supplyInfo {{
                                apy {{ value }}
                            }}
                            borrowInfo {{
                                apy {{ value }}
                                utilizationRate {{ value }}
                            }}
                            size {{
                                usd
                            }}
                        }}
                    }}
                }}
            "#, chain_id, market_address)
        });

        tracing::debug!("Fetching SparkLend rates for {:?} from Aave API", chain);

        let response = self.client
            .post(SPARKLEND_API_URL)
            .json(&query)
            .send()
            .await?;

        let response_text = response.text().await?;
        tracing::debug!("SparkLend API raw response: {}", response_text);
        
        let gql_response: GraphQLResponse = serde_json::from_str(&response_text)?;

        if let Some(errors) = &gql_response.errors {
            let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            tracing::error!("SparkLend API errors: {:?}", error_messages);
            return Err(anyhow::anyhow!("SparkLend API errors: {}", error_messages.join(", ")));
        }

        let data = gql_response.data
            .ok_or_else(|| anyhow::anyhow!("No data returned from SparkLend API"))?;
        
        if data.market.reserves.is_empty() {
            // KNOWN ISSUE: Aave v3 API does not index SparkLend markets (fork)
            // See file header for TODO on implementing alternative data source
            tracing::warn!(
                "SparkLend market {} not indexed by Aave API (expected issue - see TODO in sparklend.rs)", 
                market_address
            );
            return Ok(vec![]);
        }

        let rates = self.parse_market_data(data.market, chain)?;

        tracing::info!("Fetched {} SparkLend rates for {:?}", rates.len(), chain);

        Ok(rates)
    }

    fn parse_market_data(&self, market: Market, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let mut rates = Vec::new();

        for reserve in market.reserves {
            let symbol = &reserve.underlying_token.symbol;
            
            // Filter to supported assets
            if !SUPPORTED_ASSETS.contains(&symbol.as_str()) {
                continue;
            }

            let asset = Asset::from_symbol(symbol, "SparkLend");
            let underlying_asset_address = reserve.underlying_token.address.clone();

            let supply_apy = reserve.supply_info
                .as_ref()
                .map(|info| info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0)
                .unwrap_or(0.0);

            let borrow_apr = reserve.borrow_info
                .as_ref()
                .map(|info| info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0)
                .unwrap_or(0.0);

            let utilization_rate = reserve.borrow_info
                .as_ref()
                .and_then(|info| info.utilization_rate.as_ref())
                .map(|ur| ur.value.parse::<f64>().unwrap_or(0.0) * 100.0)
                .unwrap_or(0.0);

            let total_liquidity_usd = reserve.size.usd.parse::<f64>().unwrap_or(0.0);
            let total_liquidity = total_liquidity_usd.round() as u64;

            let utilization_decimal = utilization_rate / 100.0;
            let available_liquidity = (total_liquidity_usd * (1.0 - utilization_decimal)).round() as u64;

            rates.push(ProtocolRate {
                protocol: Protocol::SparkLend,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0, // SparkLend doesn't have additional rewards
                performance_fee: None,
                active: true,
                collateral_enabled: true,  // SparkLend supports collateral
                collateral_ltv: 0.75,
                available_liquidity,
                total_liquidity,
                utilization_rate,
                ltv: 0.75,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: Some(underlying_asset_address.clone()),
                timestamp: Utc::now(),
            });

            rates.push(ProtocolRate {
                protocol: Protocol::SparkLend,
                chain: chain.clone(),
                asset,
                action: Action::Borrow,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0,
                performance_fee: None,
                active: true,
                collateral_enabled: false,  // Borrow doesn't provide collateral
                collateral_ltv: 0.0,
                available_liquidity,
                total_liquidity,
                utilization_rate,
                ltv: 0.75,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: Some(underlying_asset_address),
                timestamp: Utc::now(),
            });
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain, underlying_asset: Option<&str>) -> String {
        let market_name = match chain {
            Chain::Ethereum => "proto_spark_v3",
            _ => "proto_spark_v3",
        };

        if let Some(asset_addr) = underlying_asset {
            format!(
                "https://app.sparkprotocol.io/reserve-overview/?underlyingAsset={}&marketName={}",
                asset_addr, market_name
            )
        } else {
            "https://app.sparkprotocol.io/".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_protocol_url() {
        let indexer = SparkLendIndexer::new();
        let url = indexer.get_protocol_url(
            &Chain::Ethereum,
            Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
        );
        assert!(url.contains("app.sparkprotocol.io"));
        assert!(url.contains("underlyingAsset="));
        assert!(url.contains("proto_spark_v3"));
    }

    #[tokio::test]
    async fn test_fetch_rates_ethereum() {
        let indexer = SparkLendIndexer::new();
        let result = indexer.fetch_rates(&Chain::Ethereum).await;
        assert!(result.is_ok(), "Failed to fetch SparkLend Ethereum rates: {:?}", result.err());
        
        let rates = result.unwrap();
        println!("SparkLend Ethereum: {} rates", rates.len());
        
        for rate in rates.iter().take(3) {
            println!("  {} {} {}: APY {:.2}%, Liquidity ${}", 
                rate.protocol, rate.chain, rate.asset, rate.supply_apy, rate.available_liquidity);
        }
    }
}
