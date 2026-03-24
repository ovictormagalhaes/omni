use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};


const AAVE_API_URL: &str = "https://api.v3.aave.com/graphql";

// Market addresses for each chain
const MARKET_ADDRESS_ARBITRUM: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";
const MARKET_ADDRESS_BASE: &str = "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5";
const MARKET_ADDRESS_ETHEREUM: &str = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";
const MARKET_ADDRESS_POLYGON: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";

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
    name: String,
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
    #[serde(rename = "usdExchangeRate")]
    usd_exchange_rate: String,
}

#[derive(Debug, Deserialize)]
struct Token {
    symbol: String,
    address: String,
    decimals: u32,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    amount: DecimalValue,
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

pub struct AaveIndexer {
    pub client: reqwest::Client,
}

impl AaveIndexer {
    pub fn new(_subgraph_arbitrum: String, _subgraph_base: String) -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        tracing::debug!("Fetching Aave rates for chain: {:?}", chain);

        let (chain_id, market_address) = match chain {
            Chain::Arbitrum => (42161, MARKET_ADDRESS_ARBITRUM),
            Chain::Base => (8453, MARKET_ADDRESS_BASE),
            Chain::Ethereum => (1, MARKET_ADDRESS_ETHEREUM),
            Chain::Polygon => (137, MARKET_ADDRESS_POLYGON),
            // Chains not supported by Aave
            _ => {
                tracing::debug!("Aave doesn't support chain {:?}, skipping", chain);
                return Ok(vec![]);
            },
        };

        // GraphQL query for Aave official API
        // TODO: Aave incentives are not available in the main reserves query
        // Need to query them separately or use on-chain data from IncentivesController
        let query = json!({
            "query": format!(r#"
                query Market {{
                    market(request: {{ chainId: {}, address: "{}" }}) {{
                        name
                        totalMarketSize
                        totalAvailableLiquidity
                        reserves {{
                            underlyingToken {{
                                symbol
                                address
                                decimals
                            }}
                            supplyInfo {{
                                apy {{ value }}
                            }}
                            borrowInfo {{
                                apy {{ value }}
                                utilizationRate {{ value }}
                            }}
                            size {{
                                amount {{ value }}
                                usd
                            }}
                            usdExchangeRate
                        }}
                    }}
                }}
            "#, chain_id, market_address)
        });

        tracing::debug!("Fetching Aave rates for {:?} from official API", chain);

        let response = self.client
            .post(AAVE_API_URL)
            .json(&query)
            .send()
            .await?;
        
        tracing::debug!("Aave API response status: {}", response.status());

        let response_text = response.text().await?;
        tracing::debug!("Aave API response: {}", response_text);

        let gql_response: GraphQLResponse = serde_json::from_str(&response_text)?;

        if let Some(errors) = &gql_response.errors {
            let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            tracing::error!("Aave API errors: {:?}", error_messages);
            return Err(anyhow::anyhow!("Aave API errors: {}", error_messages.join(", ")));
        }

        let data = gql_response.data
            .ok_or_else(|| anyhow::anyhow!("No data returned from Aave API"))?;

        let rates = self.parse_market_data(data.market, chain)?;

        tracing::info!("Fetched {} Aave rates for {:?}", rates.len(), chain);

        Ok(rates)
    }

    fn parse_market_data(&self, market: Market, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        let mut rates = Vec::new();

        for reserve in market.reserves {
            let symbol = &reserve.underlying_token.symbol;
            let asset = Asset::from_symbol(symbol, "Aave");

            // Capture the underlying asset address for URL generation
            let underlying_asset_address = reserve.underlying_token.address.clone();

            let supply_apy = reserve.supply_info
                .as_ref()
                .map(|info| info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0) // Convert to percentage
                .unwrap_or(0.0);

            let borrow_apr = reserve.borrow_info
                .as_ref()
                .map(|info| info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0) // Convert to percentage
                .unwrap_or(0.0);

            let utilization_rate = reserve.borrow_info
                .as_ref()
                .and_then(|info| info.utilization_rate.as_ref())
                .map(|ur| ur.value.parse::<f64>().unwrap_or(0.0) * 100.0) // Convert to percentage
                .unwrap_or(0.0);

            // Parse liquidity from size.usd (already in USD)
            let total_liquidity_usd = reserve.size.usd.parse::<f64>().unwrap_or(0.0);
            let total_liquidity = total_liquidity_usd.round() as u64;

            // Calculate available liquidity: total * (1 - utilization_rate)
            let utilization_decimal = utilization_rate / 100.0;
            let available_liquidity = (total_liquidity_usd * (1.0 - utilization_decimal)).round() as u64;

            rates.push(ProtocolRate {
                protocol: Protocol::Aave,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0, // Round to 2 decimals
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0, // TODO: Implement Aave incentives fetching
                performance_fee: None,
                active: true,
                collateral_enabled: true,  // Aave supply can be used as collateral
                collateral_ltv: 0.75,      // Default LTV for Aave
                available_liquidity,
                total_liquidity,
                utilization_rate,
                ltv: 0.75, // Default LTV
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: Some(underlying_asset_address.clone()),
                timestamp: Utc::now(),
            });

            rates.push(ProtocolRate {
                protocol: Protocol::Aave,
                chain: chain.clone(),
                asset,
                action: Action::Borrow,
                supply_apy: (supply_apy * 100.0).round() / 100.0,
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0, // TODO: Implement Aave incentives fetching
                performance_fee: None,
                active: true,
                collateral_enabled: false,  // Borrow action doesn't provide collateral
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
            Chain::Arbitrum => "proto_arbitrum_v3",
            Chain::Base => "proto_base_v3",
            Chain::Ethereum => "proto_mainnet_v3",
            Chain::Polygon => "proto_polygon_v3",
            _ => return String::new(),
        };

        // If we have the underlying asset address, generate specific reserve URL
        if let Some(asset_addr) = underlying_asset {
            format!(
                "https://app.aave.com/reserve-overview/?underlyingAsset={}&marketName={}",
                asset_addr.to_lowercase(), market_name
            )
        } else {
            // Fallback to generic market URL
            format!("https://app.aave.com/?marketName={}", market_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_normalization() {
        // Tests moved to models.rs Asset::from_symbol()
    }
}
