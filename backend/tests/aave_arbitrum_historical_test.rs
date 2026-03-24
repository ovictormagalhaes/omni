use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct GraphQLQuery {
    query: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<ReserveDataResponse>,
}

#[derive(Debug, Deserialize)]
struct ReserveDataResponse {
    #[serde(rename = "reserveParamsHistoryItems")]
    reserve_params_history_items: Vec<HistoricalPoint>,
}

#[derive(Debug, Deserialize)]
struct HistoricalPoint {
    timestamp: String,
    #[serde(rename = "liquidityRate")]
    liquidity_rate: String,
}

#[tokio::test]
async fn test_aave_arbitrum_usdc_has_historical_data() -> Result<()> {
    // USDC address on Arbitrum
    let usdc_address = "0xaf88d065e77c8cc2239327c5edb3a432268e5831";
    
    // Get API key from env
    let api_key = env::var("THE_GRAPH_API_KEY")
        .unwrap_or_else(|_| "13124d1f498df89160c42e0b10d17b8f".to_string());
    
    // Aave Arbitrum subgraph
    let subgraph_id = "DLuEJHDWzZCcZn61S8EyWVRYvrBdWsBxBt8aWQy6bJGW";
    let url = format!("https://gateway-arbitrum.network.thegraph.com/api/{}/subgraphs/id/{}", 
        api_key, subgraph_id);
    
    // Calculate timestamps for last 30 days
    let now = chrono::Utc::now().timestamp();
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    
    // GraphQL query
    let query = format!(r#"
        {{
            reserveParamsHistoryItems(
                first: 1000,
                orderBy: timestamp,
                orderDirection: desc,
                where: {{
                    reserve: "{}",
                    timestamp_gte: {}
                }}
            ) {{
                timestamp
                liquidityRate
            }}
        }}
    "#, usdc_address.to_lowercase(), thirty_days_ago);
    
    println!("\n🔍 Testing Aave Arbitrum USDC Historical Data");
    println!("================================================");
    println!("USDC Address: {}", usdc_address);
    println!("Subgraph URL: {}", url);
    println!("");
    
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&GraphQLQuery { query })
        .send()
        .await?;
    
    let status = response.status();
    println!("Response Status: {}", status);
    
    let response_text = response.text().await?;
    println!("Response (first 300 chars): {}", &response_text[..response_text.len().min(300)]);
    
    let response_data: GraphQLResponse = serde_json::from_str(&response_text)?;
    
    if let Some(data) = response_data.data {
        let points = data.reserve_params_history_items;
        println!("\n✅ Found {} historical data points for Arbitrum", points.len());
        
        if points.len() > 1 {
            println!("✅ CONFIRMED: Aave Arbitrum HAS historical data in The Graph");
            println!("❌ CONCLUSION: Only Aave Base is missing historical data");
        } else {
            println!("❌ Aave Arbitrum also has no historical data");
        }
    } else {
        println!("❌ No data returned");
    }
    
    Ok(())
}
