use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::models::{Asset, Chain, Protocol, ProtocolRate, Action, OperationType};
use super::RateIndexer;


const AAVE_API_URL: &str = "https://api.v3.aave.com/graphql";

// Market addresses for each chain
const MARKET_ADDRESS_ARBITRUM: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";
const MARKET_ADDRESS_BASE: &str = "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5";
const MARKET_ADDRESS_ETHEREUM: &str = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";
const MARKET_ADDRESS_POLYGON: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";
const MARKET_ADDRESS_OPTIMISM: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";
const MARKET_ADDRESS_AVALANCHE: &str = "0x794a61358D6845594F94dc1DB02A252b5b4814aD";

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
    #[allow(dead_code)]
    name: String,
    reserves: Vec<ReserveData>,
}

#[derive(Debug, Deserialize)]
struct ReserveData {
    #[serde(rename = "underlyingToken")]
    underlying_token: Token,
    #[serde(rename = "supplyInfo")]
    supply_info: SupplyInfo,
    #[serde(rename = "borrowInfo")]
    borrow_info: Option<BorrowInfo>,
    size: TokenAmount,
    #[serde(rename = "usdExchangeRate")]
    #[allow(dead_code)]
    usd_exchange_rate: String,
    #[serde(rename = "isFrozen")]
    is_frozen: bool,
    #[serde(rename = "isPaused")]
    is_paused: bool,
}

#[derive(Debug, Deserialize)]
struct Token {
    symbol: String,
    address: String,
    #[allow(dead_code)]
    decimals: u32,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    #[allow(dead_code)]
    amount: DecimalValue,
    usd: String,
}

#[derive(Debug, Deserialize)]
struct SupplyInfo {
    apy: DecimalValue,
    #[serde(rename = "canBeCollateral")]
    can_be_collateral: bool,
    #[serde(rename = "maxLTV")]
    max_ltv: PercentValue,
}

#[derive(Debug, Deserialize)]
struct BorrowInfo {
    apy: DecimalValue,
    #[serde(rename = "utilizationRate")]
    utilization_rate: PercentValue,
    #[serde(rename = "availableLiquidity")]
    available_liquidity: TokenAmount,
    #[serde(rename = "borrowingState")]
    borrowing_state: String,
    #[serde(rename = "borrowCapReached")]
    borrow_cap_reached: bool,
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
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        tracing::debug!("Fetching Aave rates for chain: {:?}", chain);

        let (chain_id, market_address) = match chain {
            Chain::Arbitrum => (42161, MARKET_ADDRESS_ARBITRUM),
            Chain::Base => (8453, MARKET_ADDRESS_BASE),
            Chain::Ethereum => (1, MARKET_ADDRESS_ETHEREUM),
            Chain::Polygon => (137, MARKET_ADDRESS_POLYGON),
            Chain::Optimism => (10, MARKET_ADDRESS_OPTIMISM),
            Chain::Avalanche => (43114, MARKET_ADDRESS_AVALANCHE),
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
                                canBeCollateral
                                maxLTV {{ value }}
                            }}
                            borrowInfo {{
                                apy {{ value }}
                                utilizationRate {{ value }}
                                availableLiquidity {{
                                    amount {{ value }}
                                    usd
                                }}
                                borrowingState
                                borrowCapReached
                            }}
                            size {{
                                amount {{ value }}
                                usd
                            }}
                            usdExchangeRate
                            isFrozen
                            isPaused
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

            let supply_apy = reserve.supply_info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0;

            let borrow_apr = reserve.borrow_info
                .as_ref()
                .map(|info| info.apy.value.parse::<f64>().unwrap_or(0.0) * 100.0)
                .unwrap_or(0.0);

            let utilization_rate = reserve.borrow_info
                .as_ref()
                .map(|info| info.utilization_rate.value.parse::<f64>().unwrap_or(0.0) * 100.0)
                .unwrap_or(0.0);

            // Parse liquidity from size.usd (already in USD)
            let total_liquidity_usd = reserve.size.usd.parse::<f64>().unwrap_or(0.0);
            let total_liquidity = total_liquidity_usd.round() as u64;

            // Use API-provided available liquidity, fallback to calculation
            let available_liquidity = reserve.borrow_info
                .as_ref()
                .map(|info| info.available_liquidity.usd.parse::<f64>().unwrap_or(0.0).round() as u64)
                .unwrap_or_else(|| {
                    let utilization_decimal = utilization_rate / 100.0;
                    (total_liquidity_usd * (1.0 - utilization_decimal)).round() as u64
                });

            // Determine active status from API fields
            let is_frozen = reserve.is_frozen;
            let is_paused = reserve.is_paused;
            let supply_active = !is_frozen && !is_paused;

            // borrowInfo is null = not borrowable at all
            // borrowingState != ENABLED = borrowing disabled
            let borrow_enabled = reserve.borrow_info
                .as_ref()
                .map(|info| info.borrowing_state == "ENABLED" && !info.borrow_cap_reached)
                .unwrap_or(false);
            let borrow_active = supply_active && borrow_enabled;

            // Use real collateral data from the API
            let collateral_enabled = reserve.supply_info.can_be_collateral;
            let collateral_ltv = reserve.supply_info.max_ltv.value.parse::<f64>().unwrap_or(0.0);

            if is_frozen {
                tracing::debug!("Aave reserve {} on {:?} is frozen, marking inactive", symbol, chain);
            }
            if !borrow_enabled {
                tracing::debug!("Aave reserve {} on {:?} borrowing disabled (borrowInfo={}, state={:?})",
                    symbol, chain,
                    reserve.borrow_info.is_some(),
                    reserve.borrow_info.as_ref().map(|i| &i.borrowing_state));
            }

            rates.push(ProtocolRate {
                protocol: Protocol::Aave,
                chain: chain.clone(),
                asset: asset.clone(),
                action: Action::Supply,
                supply_apy: (supply_apy * 100.0).round() / 100.0, // Round to 2 decimals
                borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                rewards: 0.0, // TODO: Implement Aave incentives fetching
                performance_fee: None,
                active: supply_active,
                collateral_enabled,
                collateral_ltv,
                available_liquidity,
                total_liquidity,
                utilization_rate,
                ltv: collateral_ltv,
                operation_type: OperationType::Lending,
                vault_id: None,
                vault_name: None,
                underlying_asset: Some(underlying_asset_address.clone()),
                timestamp: Utc::now(),
            });

            // Only emit borrow rate if borrowInfo exists (asset is borrowable in principle)
            if reserve.borrow_info.is_some() {
                rates.push(ProtocolRate {
                    protocol: Protocol::Aave,
                    chain: chain.clone(),
                    asset,
                    action: Action::Borrow,
                    supply_apy: (supply_apy * 100.0).round() / 100.0,
                    borrow_apr: (borrow_apr * 100.0).round() / 100.0,
                    rewards: 0.0, // TODO: Implement Aave incentives fetching
                    performance_fee: None,
                    active: borrow_active,
                    collateral_enabled: false,
                    collateral_ltv: 0.0,
                    available_liquidity,
                    total_liquidity,
                    utilization_rate,
                    ltv: collateral_ltv,
                    operation_type: OperationType::Lending,
                    vault_id: None,
                    vault_name: None,
                    underlying_asset: Some(underlying_asset_address),
                    timestamp: Utc::now(),
                });
            }
        }

        Ok(rates)
    }

    pub fn get_protocol_url(&self, chain: &Chain, underlying_asset: Option<&str>) -> String {
        let market_name = match chain {
            Chain::Arbitrum => "proto_arbitrum_v3",
            Chain::Base => "proto_base_v3",
            Chain::Ethereum => "proto_mainnet_v3",
            Chain::Polygon => "proto_polygon_v3",
            Chain::Optimism => "proto_optimism_v3",
            Chain::Avalanche => "proto_avalanche_v3",
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

#[async_trait]
impl RateIndexer for AaveIndexer {
    fn protocol(&self) -> Protocol {
        Protocol::Aave
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

    async fn fetch_rates(&self, chain: &Chain) -> Result<Vec<ProtocolRate>> {
        self.fetch_rates(chain).await
    }

    fn rate_url(&self, rate: &ProtocolRate) -> String {
        self.get_protocol_url(&rate.chain, rate.underlying_asset.as_deref())
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
